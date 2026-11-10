package com.fest.visuals.client.ui.widget.overlay;

import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.blaze3d.vertex.PoseStack;

import java.awt.Color;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;

import com.fest.visuals.api.module.Module;
import com.fest.visuals.api.module.ModuleManager;
import com.fest.visuals.api.system.media.NowPlayingService;
import com.fest.visuals.api.utils.animation.AnimationUtil;
import com.fest.visuals.api.utils.animation.Easing;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.math.MouseUtil;
import com.fest.visuals.api.utils.render.McText;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.api.utils.render.fonts.Fonts;
import com.fest.visuals.api.utils.render.fonts.Icons;
import com.fest.visuals.client.features.modules.hud.DynamicIslandModule;
import com.fest.visuals.client.ui.widget.Widget;

import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;

/**
 * The island sits at the top of the screen and has three faces. Idle it is a status bar:
 * clock, brand pill with a live dot, wifi and ping. While a media session plays it morphs in
 * place into a now-playing card with real cover art, a waveform driven by the output peak
 * meter, live position and working transport controls. Module toggles stack underneath it as
 * fading toasts.
 */
public class DynamicIslandWidget extends Widget {
    private static final DateTimeFormatter TIME_FMT = DateTimeFormatter.ofPattern("HH:mm");
    private static final Identifier ART_ID = Identifier.fromNamespaceAndPath("festvisuals", "media/album_art");

    /** Scroll span and the hold at each end, for titles too long for the column. */
    private static final Duration MARQUEE_CYCLE = Duration.ofMillis(4200);
    private static final Duration MARQUEE_PAUSE = Duration.ofMillis(1500);

    private static final long FADE_MS = 380L;

    /** 0 = status bar, 1 = music card. */
    private final AnimationUtil morph = new AnimationUtil();

    /** Smoothed per-bar heights so the waveform does not flicker between frames. */
    private final float[] waveLevels = new float[NowPlayingService.WAVE_SAMPLES];

    private DynamicTexture artTexture;
    private int loadedArtVersion = -1;
    private boolean artReady;
    /** Centred square crop of the cover, in normalised UVs. */
    private float artU, artV, artUW = 1f, artVH = 1f;

    /** Last rendered transport hitboxes, in GUI space. Null while the card is not interactive. */
    private Hitbox previousHit, playHit, nextHit;

    private final Map<Module, Boolean> lastStates = new HashMap<>();
    private final Deque<Toast> toasts = new ArrayDeque<>();
    private boolean seeded;

    private record Hitbox(float x, float y, float size) {
        boolean contains(double mouseX, double mouseY) {
            return MouseUtil.isHovered(mouseX, mouseY, x, y, size, size);
        }
    }

    private static final class Toast {
        final String name;
        final boolean on;
        final long addedAt;

        Toast(String name, boolean on) {
            this.name = name;
            this.on = on;
            this.addedAt = System.currentTimeMillis();
        }
    }

    public DynamicIslandWidget() {
        super(500f, 6f);
    }

    @Override
    public String getName() { return "Dynamic Island"; }

    @Override
    public float fontMul() { return DynamicIslandModule.getInstance().fontScale.getValue(); }

    /** Design units: the reference card is 372x140, rendered here at ~0.48 scale. */
    private float d(float designPx) { return scaled(designPx * 0.48f); }

    private static float lerp(float a, float b, float t) { return a + (b - a) * t; }

    private int ping() {
        if (mc.player == null || mc.player.connection == null) return 0;
        PlayerInfo info = mc.player.connection.getPlayerInfo(mc.player.getUUID());
        return info == null ? 0 : info.getLatency();
    }

    @Override
    public void onMouseClick(double mouseX, double mouseY, int button) {
        if (button != 0) return;

        NowPlayingService service = NowPlayingService.getInstance();
        if (previousHit != null && previousHit.contains(mouseX, mouseY)) service.skipPrevious();
        else if (playHit != null && playHit.contains(mouseX, mouseY)) service.togglePlayPause();
        else if (nextHit != null && nextHit.contains(mouseX, mouseY)) service.skipNext();
        else return;

        // The card overlaps the drag box, so cancel the drag this click would have started.
        getDraggable().setDragging(false);
    }

    @Override
    public void render(PoseStack matrixStack) {
        FestRenderer.withBackdrop(() -> renderIsland(matrixStack));
    }

    private void renderIsland(PoseStack matrixStack) {
        NowPlayingService service = NowPlayingService.getInstance();
        NowPlayingService.NowPlaying np = service.current();
        
        boolean hasMusic = false;
        String _islandMode = DynamicIslandModule.getInstance().displayMode.getValue();
        if (_islandMode.equals("Только Остров")) {
            hasMusic = false;
        } else if (_islandMode.equals("Только Плеер")) {
            hasMusic = true;
        } else {
            hasMusic = np != null && np.hasTrack();
        }

        morph.update();
        morph.run(hasMusic ? 1.0 : 0.0, 520, Easing.EXPO_OUT);
        float m = (float) morph.getValue();

        float barW = statusWidth();
        float barH = statusHeight();
        float cardW = d(372f);
        float cardH = d(140f);

        // Anchored on the status bar's centre so the card grows symmetrically from the pill
        // wherever the player has dragged it.
        float centreX = getDraggable().getX() + barW / 2f;
        float y = getDraggable().getY();

        float w = lerp(barW, cardW, m);
        float h = lerp(barH, cardH, m);
        float radius = lerp(barH / 2f, d(30f), m);
        float x = centreX - w / 2f;

        if (m > 0.01f) {
            RenderUtil.BLUR_RECT.draw(matrixStack, x, y, w, h, radius, ColorUtil.setAlpha(Color.BLACK, (int) (235 * m)), 0.28f);
        }

        if (m < 0.99f) renderStatusBar(matrixStack, centreX - barW / 2f, y, barH, 1f - m);

        previousHit = playHit = nextHit = null;
        if (m > 0.01f && hasMusic) renderMusicCard(matrixStack, service, np, x, y, w, h, m);

        // Notifications are pinned to the screen, not to the island: dragging the island around
        // must not drag them with it.
        renderToasts(matrixStack,
                mc.getWindow().getGuiScaledWidth() / 2f,
                mc.getWindow().getGuiScaledHeight() / 2f + scaled(14f));

        // Drag box stays the status bar so the anchor never drifts while morphing.
        getDraggable().setWidth(barW);
        getDraggable().setHeight(barH);
    }

    // ---------------------------------------------------------------- status bar

    private float statusFont() { return scaled(6.5f * fontMul()); }
    private float statusHeight() { return statusFont() + scaled(5f); }

    private float statusWidth() {
        DynamicIslandModule cfg = DynamicIslandModule.getInstance();
        float fontSize = statusFont();
        float pad = scaled(6f);
        float gap = scaled(6f);
        float dot = scaled(2f);

        float clockW = cfg.showClock.getValue() ? McText.getWidth(clockText(), fontSize) : 0f;
        float pillW = McText.getWidth("Fest", fontSize) + McText.getWidth("Visuals", fontSize)
                + gap + dot + pad * 2f;
        float pingW = 0f;
        if (cfg.showPing.getValue()) {
            pingW = Fonts.ICONS.getWidth(Icons.WLAN.getLetter(), fontSize)
                    + scaled(2.5f)
                    + McText.getWidth(pingText(), fontSize);
        }

        return clockW + (cfg.showClock.getValue() ? gap : 0f) + pillW + (cfg.showPing.getValue() ? gap + pingW : 0f);
    }

    private String clockText() { return LocalTime.now().format(TIME_FMT); }
    private String pingText() { return ping() + " мс"; }

    private void renderStatusBar(PoseStack matrixStack, float x, float y, float pillH, float a) {
        DynamicIslandModule cfg = DynamicIslandModule.getInstance();

        float fontSize = statusFont();
        float pad = scaled(6f);
        float gap = scaled(6f);
        float dot = scaled(2f);

        String clock = clockText();
        String brandLeft = "Fest";
        String brandRight = "Visuals";
        String ping = pingText();

        float clockW = McText.getWidth(clock, fontSize);
        float brandLeftW = McText.getWidth(brandLeft, fontSize);
        float brandRightW = McText.getWidth(brandRight, fontSize);
        float pillW = brandLeftW + brandRightW + gap + dot + pad * 2f;

        Color clockColor = ColorUtil.setAlpha(new Color(248, 248, 250), (int) (255 * a));
        Color textLight = ColorUtil.setAlpha(new Color(242, 242, 244), (int) (240 * a));
        Color accent = UIColors.primary((int) (255 * a));

        float cx = x;
        float baselineY = y + (pillH - fontSize) / 2f;

        if (cfg.showClock.getValue()) {
            McText.drawText(matrixStack, clock, cx, baselineY, fontSize, clockColor);
            cx += clockW + gap;
        }

        RenderUtil.BLUR_RECT.draw(matrixStack, cx, y, pillW, pillH, pillH / 2f, ColorUtil.setAlpha(Color.BLACK, (int) (235 * a)), 0.28f);
        float bx = cx + pad;
        McText.drawText(matrixStack, brandLeft, bx, baselineY, fontSize, textLight);
        McText.drawText(matrixStack, brandRight, bx + brandLeftW, baselineY, fontSize, accent);

        float dotX = bx + brandLeftW + brandRightW + gap - dot / 2f + scaled(2f);
        float dotY = y + pillH / 2f - dot / 2f;
        RenderUtil.RECT.draw(matrixStack, dotX, dotY, dot, dot, dot / 2f, ColorUtil.setAlpha(new Color(58, 208, 127), (int) (255 * a)));
        cx += pillW;

        if (cfg.showPing.getValue()) {
            cx += gap;
            Color pingColor = UIColors.primary((int) (240 * a));
            Fonts.ICONS.drawText(matrixStack, Icons.WLAN.getLetter(), cx, baselineY, fontSize, pingColor);
            cx += Fonts.ICONS.getWidth(Icons.WLAN.getLetter(), fontSize) + scaled(2.5f);
            McText.drawText(matrixStack, ping, cx, baselineY, fontSize, pingColor);
        }
    }

    // ---------------------------------------------------------------- music card

    private void renderMusicCard(PoseStack matrixStack, NowPlayingService service,
                                 NowPlayingService.NowPlaying np,
                                 float x, float y, float w, float h, float a) {
        int alpha = (int) (255 * a);

        float padX = d(18f);
        float padBottom = d(16f);
        float headerH = d(10f);

        float albumSize = d(62f);
        float albumRound = d(15f);
        float columnGap = d(15f);

        float artistFont = d(11.5f);
        float titleFont = d(16f);
        float timeFont = d(10.5f);

        float contentY = y + headerH + d(4f);
        float albumX = x + padX;

        renderAlbumArt(matrixStack, service, albumX, contentY, albumSize, albumRound, a);

        // ---- text column
        float textX = albumX + albumSize + columnGap;
        float textW = Math.max(d(40f), (x + w - padX) - textX);

        String artist = truncate(np.artist().isEmpty() ? np.source() : np.artist(), textW, artistFont);
        String title = np.title().isEmpty() ? "Unknown track" : np.title();

        float artistY = contentY + d(2f);
        McText.drawText(matrixStack, artist, textX, artistY, artistFont,
                ColorUtil.setAlpha(Color.WHITE, (int) (102 * a)));

        // Track names routinely overflow the column, so scroll rather than clip them.
        // drawWrap falls back to a plain draw whenever the text already fits.
        float titleY = artistY + artistFont + d(5f);
        McText.drawWrap(matrixStack, title, textX, titleY, textW, titleFont,
                ColorUtil.setAlpha(new Color(244, 244, 246), alpha), d(24f),
                MARQUEE_CYCLE, MARQUEE_PAUSE);

        float waveY = titleY + titleFont + d(6f);
        renderWave(matrixStack, service, textX, waveY, textW, d(18f), np.playing(), a);

        // ---- transport row, anchored to the bottom; the progress row sits just above it
        float controlSize = d(18f);
        float controlY = y + h - padBottom - controlSize;
        renderTransport(matrixStack, np, x + w / 2f, controlY, controlSize, a);

        // ---- progress row
        int position = service.positionMs();
        int duration = np.durationMs();

        float barH = d(4f);
        float barY = controlY - d(12f) - barH;
        String elapsed = formatTime(position);
        String remaining = duration > 0 ? "-" + formatTime(Math.max(0, duration - position)) : "";
        float elapsedW = McText.getWidth(elapsed, timeFont);
        float remainingW = remaining.isEmpty() ? 0f : McText.getWidth(remaining, timeFont);
        Color timeColor = ColorUtil.setAlpha(Color.WHITE, (int) (102 * a));

        float timeY = barY + barH / 2f - timeFont / 2f;
        McText.drawText(matrixStack, elapsed, x + padX, timeY, timeFont, timeColor);
        if (!remaining.isEmpty()) {
            McText.drawText(matrixStack, remaining, x + w - padX - remainingW, timeY, timeFont, timeColor);
        }

        float trackX = x + padX + elapsedW + d(10f);
        float trackEnd = x + w - padX - (remaining.isEmpty() ? 0f : remainingW + d(10f));
        float trackW = Math.max(0f, trackEnd - trackX);
        RenderUtil.RECT.draw(matrixStack, trackX, barY, trackW, barH, barH / 2f,
                ColorUtil.setAlpha(Color.WHITE, (int) (26 * a)));

        float progress = duration > 0 ? Math.min(1f, (float) position / duration) : 0f;
        if (progress > 0f) {
            Color fillA = ColorUtil.setAlpha(UIColors.secondary(), alpha);
            Color fillB = ColorUtil.setAlpha(UIColors.primary(), alpha);
            RenderUtil.GRADIENT_RECT.draw(matrixStack, trackX, barY, trackW * progress, barH, barH / 2f,
                    fillA, fillB, fillA, fillB);
        }
    }

    /** Previous / play-pause / next, centred on {@code centreX}. Records its own hitboxes. */
    private void renderTransport(PoseStack matrixStack, NowPlayingService.NowPlaying np,
                                 float centreX, float y, float size, float a) {
        float sideSize = size * 0.78f;
        float spacing = d(26f);
        float centreY = y + size / 2f;

        previousHit = drawControl(matrixStack, Icons.STEP_B.getLetter(), centreX - spacing, centreY, sideSize, a, false);
        playHit = drawControl(matrixStack, np.playing() ? Icons.PAUSE.getLetter() : Icons.PLAY.getLetter(),
                centreX, centreY, size, a, true);
        nextHit = drawControl(matrixStack, Icons.STEP_F.getLetter(), centreX + spacing, centreY, sideSize, a, false);
    }

    private Hitbox drawControl(PoseStack matrixStack, String glyph, float centreX, float centreY,
                               float size, float a, boolean primary) {
        float touch = size + d(10f);
        Hitbox box = new Hitbox(centreX - touch / 2f, centreY - touch / 2f, touch);

        boolean hovered = box.contains(mouseX(), mouseY());
        if (hovered) {
            RenderUtil.RECT.draw(matrixStack, box.x(), box.y(), touch, touch, touch / 2f,
                    ColorUtil.setAlpha(Color.WHITE, (int) (26 * a)));
        }

        int base = primary ? 230 : 150;
        float glyphWidth = Fonts.ICONS.getWidth(glyph, size);
        Fonts.ICONS.drawText(matrixStack, glyph, centreX - glyphWidth / 2f, centreY - size / 2f, size,
                ColorUtil.setAlpha(Color.WHITE, (int) ((hovered ? 255 : base) * a)));

        // Only accept clicks once the card is essentially settled, so a half-morphed
        // control cannot be hit where it is not yet drawn.
        return a > 0.85f ? box : null;
    }

    // ---------------------------------------------------------------- album art

    private void renderAlbumArt(PoseStack matrixStack, NowPlayingService service,
                                float x, float y, float size, float round, float a) {
        syncArtTexture(service);

        if (artReady) {
            RenderUtil.TEXTURE_RECT.draw(matrixStack, x, y, size, size, round,
                    ColorUtil.setAlpha(Color.WHITE, (int) (255 * a)), artU, artV, artUW, artVH, ART_ID);
            return;
        }

        // Fallback tile: accent gradient with vinyl rings.
        Color top = ColorUtil.setAlpha(UIColors.primary(), (int) (150 * a));
        Color bottom = ColorUtil.setAlpha(new Color(26, 23, 32), (int) (255 * a));
        RenderUtil.GRADIENT_RECT.draw(matrixStack, x, y, size, size, round, top, bottom, bottom, bottom);

        drawCentredDisc(matrixStack, x, y, size, size * 0.52f, ColorUtil.setAlpha(Color.BLACK, (int) (110 * a)));
        drawCentredDisc(matrixStack, x, y, size, size * 0.34f, ColorUtil.setAlpha(Color.WHITE, (int) (18 * a)));
        drawCentredDisc(matrixStack, x, y, size, size * 0.14f, ColorUtil.setAlpha(UIColors.primary(), (int) (220 * a)));
    }

    /** Uploads the cover the media service dropped on disk, once per track change. */
    private void syncArtTexture(NowPlayingService service) {
        int version = service.artVersion();
        if (version == loadedArtVersion) return;
        loadedArtVersion = version;

        releaseArt();

        String path = service.artPath();
        if (path == null || path.isEmpty()) return;

        try {
            Path file = Paths.get(path);
            if (!Files.isReadable(file)) return;

            NativeImage image = NativeImage.read(Files.readAllBytes(file));

            // Covers are not always square (browser art is often 16:9); crop to the centre.
            int w = image.getWidth();
            int h = image.getHeight();
            if (w > h) {
                artUW = (float) h / w;
                artU = (1f - artUW) / 2f;
                artV = 0f;
                artVH = 1f;
            } else {
                artVH = (float) w / h;
                artV = (1f - artVH) / 2f;
                artU = 0f;
                artUW = 1f;
            }

            artTexture = new DynamicTexture(() -> "FestVisuals album art", image);
            mc.getTextureManager().register(ART_ID, artTexture);
            artReady = true;
        } catch (IOException | RuntimeException e) {
            artReady = false;
        }
    }

    private void releaseArt() {
        if (artTexture != null) {
            mc.getTextureManager().release(ART_ID);
            artTexture.close();
            artTexture = null;
        }
        artReady = false;
    }

    private void drawCentredDisc(PoseStack matrixStack, float tileX, float tileY, float tileSize, float size, Color color) {
        RenderUtil.RECT.draw(matrixStack,
                tileX + (tileSize - size) / 2f,
                tileY + (tileSize - size) / 2f,
                size, size, size / 2f, color);
    }

    // ---------------------------------------------------------------- waveform

    /**
     * Renders the tail of the peak-meter ring buffer. Bars are eased toward their target so a
     * 20 Hz sample stream still looks smooth at render framerate.
     */
    private void renderWave(PoseStack matrixStack, NowPlayingService service,
                            float x, float y, float w, float h, boolean playing, float a) {
        // Fixed thin bars on a fixed pitch, as in the design. Deriving the width from the
        // available span instead makes them fat and short, which reads as a row of dots.
        float barW = d(3f);
        float pitch = d(6f);
        float barRound = barW * 0.25f;

        int count = Math.min(waveLevels.length, Math.max(1, (int) (w / pitch)));

        for (int i = 0; i < count; i++) {
            // Newest sample on the right: bar 0 is the oldest of the retained window.
            float sample = playing ? service.waveSample(count - 1 - i) : 0f;

            // Peak values are heavily bottom-weighted; the square root opens up quiet passages.
            float target = 0.15f + 0.85f * (float) Math.sqrt(Math.min(1f, sample));
            waveLevels[i] += (target - waveLevels[i]) * 0.35f;

            float barH = h * Math.max(0.15f, waveLevels[i]);
            boolean strong = i % 3 == 0;
            int barAlpha = (int) ((strong ? 255 : 115) * a);

            RenderUtil.RECT.draw(matrixStack, x + i * pitch, y + h - barH, barW, barH, barRound,
                    ColorUtil.setAlpha(UIColors.primary(), barAlpha));
        }
    }

    // ---------------------------------------------------------------- toasts

    private long lifeMs() { return (long) (DynamicIslandModule.getInstance().lifetime.getValue() * 1000f); }
    private int maxToasts() { return DynamicIslandModule.getInstance().maxToasts.getValue().intValue(); }
    private boolean showState() { return DynamicIslandModule.getInstance().showState.getValue(); }

    private void pollModules() {
        for (Module module : ModuleManager.getInstance().getModules()) {
            Boolean prev = lastStates.get(module);
            boolean cur = module.isEnabled();
            if (prev == null) {
                lastStates.put(module, cur);
                continue;
            }
            if (prev != cur) {
                lastStates.put(module, cur);
                if (seeded) {
                    toasts.addFirst(new Toast(module.getName(), cur));
                    while (toasts.size() > maxToasts()) toasts.removeLast();
                }
            }
        }
        seeded = true;
    }

    private float toastAlpha(Toast t) {
        long age = System.currentTimeMillis() - t.addedAt;
        if (age < FADE_MS) return age / (float) FADE_MS;
        if (age > lifeMs()) return Math.max(0f, 1f - (age - lifeMs()) / (float) FADE_MS);
        return 1f;
    }

    /** Toggle notifications, stacked directly under the crosshair. */
    private void renderToasts(PoseStack matrixStack, float centreX, float topY) {
        pollModules();

        long now = System.currentTimeMillis();
        Iterator<Toast> expiry = toasts.iterator();
        while (expiry.hasNext()) {
            if (now - expiry.next().addedAt > lifeMs() + FADE_MS) expiry.remove();
        }
        if (toasts.isEmpty()) return;

        float pad = scaled(6f);
        float iconSize = scaled(8f);
        float gap = scaled(6f);
        float fontSize = scaled(6.5f * fontMul());
        float pillH = fontSize + pad * 1.6f;
        float rowGap = scaled(3f);

        float rowY = topY;
        for (Toast t : toasts) {
            float alpha = toastAlpha(t);
            if (alpha <= 0f) {
                rowY += pillH + rowGap;
                continue;
            }
            int a = (int) (alpha * 255f);

            String state = showState() ? (t.on ? "вкл" : "выкл") : "";
            float labelW = getMediumFont().getWidth(t.name, fontSize);
            float stateW = state.isEmpty() ? 0f : getMediumFont().getWidth(state, fontSize);
            float rowW = pad + iconSize + gap + labelW + (stateW > 0 ? gap + stateW : 0) + pad;
            float rowX = centreX - rowW / 2f;

            RenderUtil.BLUR_RECT.draw(matrixStack, rowX, rowY, rowW, pillH, pillH / 2f, UIColors.widgetBlur(a), 0.28f);

            float iconX = rowX + pad;
            float iconY = rowY + (pillH - iconSize) / 2f;
            Color tint = t.on
                    ? ColorUtil.setAlpha(UIColors.primary(), (int) (0.22f * a))
                    : ColorUtil.setAlpha(Color.WHITE, (int) (0.08f * a));
            RenderUtil.RECT.draw(matrixStack, iconX, iconY, iconSize, iconSize, iconSize / 2f, tint);

            float dot = iconSize * 0.42f;
            Color dotColor = t.on ? UIColors.primary(a) : UIColors.inactiveTextColor((int) (0.5f * a));
            RenderUtil.RECT.draw(matrixStack, iconX + (iconSize - dot) / 2f, iconY + (iconSize - dot) / 2f, dot, dot, dot / 2f, dotColor);

            float textY = rowY + (pillH - fontSize) / 2f;
            getMediumFont().drawText(matrixStack, t.name, iconX + iconSize + gap, textY, fontSize, UIColors.textColor(a));

            Color stateColor = t.on ? UIColors.primary(a) : UIColors.inactiveTextColor((int) (0.55f * a));
            getMediumFont().drawText(matrixStack, state, rowX + rowW - pad - stateW, textY, fontSize, stateColor);

            rowY += pillH + rowGap;
        }
    }

    // ---------------------------------------------------------------- helpers

    private double mouseX() { return mc.mouseHandler.xpos() / mc.getWindow().getGuiScale(); }
    private double mouseY() { return mc.mouseHandler.ypos() / mc.getWindow().getGuiScale(); }

    private String truncate(String text, float maxWidth, float size) {
        if (text == null || text.isEmpty()) return "";
        if (McText.getWidth(text, size) <= maxWidth) return text;

        String cut = text;
        while (cut.length() > 1 && McText.getWidth(cut + "вЂ¦", size) > maxWidth) {
            cut = cut.substring(0, cut.length() - 1);
        }
        return cut + "вЂ¦";
    }

    private String formatTime(int millis) {
        int total = Math.max(0, millis) / 1000;
        return (total / 60) + ":" + String.format("%02d", total % 60);
    }
}
