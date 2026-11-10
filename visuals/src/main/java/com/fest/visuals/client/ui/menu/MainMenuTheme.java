package com.fest.visuals.client.ui.menu;

import com.mojang.blaze3d.platform.Window;
import com.mojang.blaze3d.vertex.PoseStack;
import lombok.experimental.UtilityClass;

import java.awt.Color;
import java.time.LocalDate;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.IdentityHashMap;
import java.util.Locale;
import java.util.Map;

import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.render.KawaseBlurProgram;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.fonts.Fonts;
import com.fest.visuals.api.utils.render.fonts.Icons;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.api.utils.render.pipeline.FestTextures;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.resources.Identifier;
import org.lwjgl.glfw.GLFW;

/**
 * Title screen skin: wallpaper, clock, wordmark, the two mode cards and the corner exit pill.
 *
 * <p>Everything is laid out against a 1920x1080 reference and scaled by the window height, so the
 * proportions of the design survive at any resolution and interface scale. The mixins place the
 * vanilla widgets on those coordinates and this class paints them.
 */
@UtilityClass
public class MainMenuTheme {
    private final Identifier BACKGROUND =
            Identifier.fromNamespaceAndPath("festvisuals", "images/menu_background.png");
    private final Identifier LOGO =
            Identifier.fromNamespaceAndPath("festvisuals", "images/logo.png");

    private final Identifier CARD_SINGLE_ART =
            Identifier.fromNamespaceAndPath("festvisuals", "images/card_singleplayer.png");
    private final Identifier CARD_MULTI_ART =
            Identifier.fromNamespaceAndPath("festvisuals", "images/card_multiplayer.png");

    /** Native sizes of the card art, needed to crop rather than stretch it. */
    private final float CARD_SINGLE_ASPECT = 735f / 420f;
    private final float CARD_MULTI_ASPECT = 735f / 490f;

    private final float BG_WIDTH = 1198f;
    private final float BG_HEIGHT = 672f;
    private final float LOGO_ASPECT = 217f / 1492f;

    /** The design canvas the pixel values below are written against. */
    private final float REFERENCE_HEIGHT = 1080f;

    private final Color AMBER_300 = new Color(255, 190, 74);
    private final Color AMBER_500 = new Color(242, 117, 22);
    private final Color AMBER_700 = new Color(138, 44, 16);
    private final Color INK_900 = new Color(16, 19, 31);

    /** Scrim over the title screen: light, the amber gradients below add most of the depth. */
    public final int TITLE_SCRIM = 90;

    /** Scrim over list and settings screens, which are dense with small text. */
    public final int MENU_SCRIM = 165;

    /** What each vanilla widget was repurposed into, filled by the title screen mixin. */
    public enum Role { CARD_SINGLE, CARD_MULTI, PILL_SETTINGS, PILL_PACKS }

    private final Map<AbstractWidget, Role> roles = new IdentityHashMap<>();

    private long exitHoldStart;

    public void clearRoles() {
        roles.clear();
    }

    public void assign(AbstractWidget widget, Role role) {
        roles.put(widget, role);
    }

    public Role roleOf(AbstractWidget widget) {
        return roles.get(widget);
    }

    // ---------------------------------------------------------------- geometry

    public float unit() {
        return Minecraft.getInstance().getWindow().getGuiScaledHeight() / REFERENCE_HEIGHT;
    }

    /** Reference pixels to GUI units. */
    public float d(float designPx) {
        return designPx * unit();
    }

    public float cardWidth()  { return d(420f); }
    public float cardHeight() { return d(254f); }
    public float cardGap()    { return d(28f); }
    public float pillHeight() { return d(37f); }
    public float pillGap()    { return d(14f); }

    /** Height of the whole centre column, used to centre it vertically. */
    public float stageHeight() {
        return d(96f + 6f + 16f + 32f + 114f + 44f) + cardHeight() + d(26f) + pillHeight();
    }

    public float stageTop() {
        float screenHeight = Minecraft.getInstance().getWindow().getGuiScaledHeight();
        return Math.max(d(12f), (screenHeight - stageHeight()) / 2f);
    }

    public float cardsTop() {
        return stageTop() + d(96f + 6f + 16f + 32f + 114f + 44f);
    }

    // ---------------------------------------------------------------- background

    /**
     * Builds the menu Kawase once from the wallpaper. Panels then sample that blur, the same
     * targets the in-world glass uses, instead of a sharp crop.
     */
    public void prepareGlassBlur() {
        Window window = Minecraft.getInstance().getWindow();
        float screenW = Math.max(1f, window.getGuiScaledWidth());
        float screenH = Math.max(1f, window.getGuiScaledHeight());
        float scale = Math.max(screenW / BG_WIDTH, screenH / BG_HEIGHT);
        float visibleU = screenW / (BG_WIDTH * scale);
        float visibleV = screenH / (BG_HEIGHT * scale);
        KawaseBlurProgram.renderMenu(FestTextures.view(BACKGROUND),
                (1f - visibleU) / 2f, (1f - visibleV) / 2f, visibleU, visibleV);
    }

    /** Dark liquid glass over the cached menu blur. */
    public void glass(PoseStack matrixStack, float x, float y, float w, float h, float radius, Color tint, float mix, float strength) {
        RenderUtil.BLUR_RECT.draw(matrixStack, x, y, w, h, radius, tint, mix);
    }

    /**
     * Wallpaper plus the design's amber warmth and vignette. Drawn into the backdrop queue so it
     * lands under everything the screen puts on top.
     */
    public void renderBackground(PoseStack matrixStack, float width, float height, int scrim) {
        FestRenderer.withBackdrop(() -> {
            // Cover, not stretch: crop the overflowing axis instead of squashing the image.
            float scale = Math.max(width / BG_WIDTH, height / BG_HEIGHT);
            float visibleU = width / (BG_WIDTH * scale);
            float visibleV = height / (BG_HEIGHT * scale);

            RenderUtil.TEXTURE_RECT.draw(matrixStack, 0f, 0f, width, height, 0f, Color.WHITE,
                    (1f - visibleU) / 2f, (1f - visibleV) / 2f, visibleU, visibleV, BACKGROUND);

            RenderUtil.RECT.draw(matrixStack, 0f, 0f, width, height, 0f,
                    ColorUtil.setAlpha(Color.BLACK, scrim));

            // Warm the lower half and darken the very top and bottom, as in the design.
            RenderUtil.GRADIENT_RECT.draw(matrixStack, 0f, height * 0.45f, width, height * 0.55f, 0f,
                    ColorUtil.setAlpha(AMBER_700, 0), ColorUtil.setAlpha(AMBER_700, 0),
                    ColorUtil.setAlpha(AMBER_700, 70), ColorUtil.setAlpha(AMBER_700, 70));

            RenderUtil.GRADIENT_RECT.draw(matrixStack, 0f, 0f, width, height * 0.3f, 0f,
                    ColorUtil.setAlpha(Color.BLACK, 140), ColorUtil.setAlpha(Color.BLACK, 140),
                    ColorUtil.setAlpha(Color.BLACK, 0), ColorUtil.setAlpha(Color.BLACK, 0));

            RenderUtil.GRADIENT_RECT.draw(matrixStack, 0f, height * 0.7f, width, height * 0.3f, 0f,
                    ColorUtil.setAlpha(Color.BLACK, 0), ColorUtil.setAlpha(Color.BLACK, 0),
                    ColorUtil.setAlpha(Color.BLACK, 165), ColorUtil.setAlpha(Color.BLACK, 165));
        });
    }

    // ---------------------------------------------------------------- centre stage

    /** Clock, date and wordmark. The cards and pills are widgets and paint themselves. */
    public void renderStage(PoseStack matrixStack, float screenWidth) {
        float centreX = screenWidth / 2f;
        float y = stageTop();

        String clock = LocalTime.now().format(DateTimeFormatter.ofPattern("HH:mm"));
        float clockSize = d(96f);
        Fonts.PS_BOLD.drawCenteredText(matrixStack, clock, centreX, y, clockSize, Color.WHITE);

        String date = LocalDate.now()
                .format(DateTimeFormatter.ofPattern("EEEE, MMMM d", Locale.ENGLISH));
        float dateSize = d(16f);
        float dateY = y + clockSize + d(6f);
        Fonts.PS_MEDIUM.drawCenteredText(matrixStack, date, centreX, dateY, dateSize,
                ColorUtil.setAlpha(Color.WHITE, 190));

        float logoHeight = d(114f);
        float logoWidth = logoHeight / LOGO_ASPECT;
        float logoY = dateY + dateSize + d(32f);
        RenderUtil.TEXTURE_RECT.draw(matrixStack, centreX - logoWidth / 2f, logoY, logoWidth, logoHeight,
                0f, Color.WHITE, 0f, 0f, 1f, 1f, LOGO);
    }

    // ---------------------------------------------------------------- cards

    /** Mode card: header with an icon tile, then a preview pane cropped from the wallpaper. */
    public void renderCard(PoseStack matrixStack, AbstractWidget widget, Role role) {
        float x = widget.getX();
        float y = widget.getY();
        float w = widget.getWidth();
        float h = widget.getHeight();

        boolean hovered = widget.isHoveredOrFocused();
        // The design lifts the card on hover; shifting the paint keeps the hitbox stable.
        float lift = hovered ? -d(4f) : 0f;
        y += lift;

        float radius = d(14f);
        float pad = d(14f);

        glass(matrixStack, x, y, w, h, radius, ColorUtil.setAlpha(new Color(20, 15, 10), 220), 0.28f, 1f);

        outline(matrixStack, x, y, w, h, radius, ColorUtil.setAlpha(AMBER_300, hovered ? 165 : 90), d(1f));

        // ---- header
        float iconSize = d(28f);
        float headerY = y + pad + d(6f);
        String title = role == Role.CARD_SINGLE ? "SinglePlayer" : "MultiPlayer";
        float titleSize = d(16f);

        Fonts.PS_BOLD.drawText(matrixStack, title, x + pad + d(8f),
                headerY + (iconSize - titleSize) / 2f, titleSize, Color.WHITE);

        float iconX = x + w - pad - d(8f) - iconSize;
        RenderUtil.RECT.draw(matrixStack, iconX, headerY, iconSize, iconSize, d(6f),
                ColorUtil.setAlpha(AMBER_300, 36));
        outline(matrixStack, iconX, headerY, iconSize, iconSize, d(6f), ColorUtil.setAlpha(AMBER_300, 90), d(1f));

        String glyph = (role == Role.CARD_SINGLE ? Icons.SINGLEPLAYER : Icons.MULTIPLAYER).getLetter();
        float glyphSize = iconSize * 0.5f;
        float glyphWidth = Fonts.ICONS.getWidth(glyph, glyphSize);
        Fonts.ICONS.drawText(matrixStack, glyph, iconX + (iconSize - glyphWidth) / 2f,
                headerY + (iconSize - glyphSize) / 2f, glyphSize, AMBER_300);

        // ---- preview
        float previewY = headerY + iconSize + d(12f);
        float previewH = y + h - pad - previewY;
        float previewW = w - pad * 2f;
        float previewX = x + pad;
        float previewRadius = d(10f);

        boolean singlePlayer = role == Role.CARD_SINGLE;
        Identifier art = singlePlayer ? CARD_SINGLE_ART : CARD_MULTI_ART;
        float artAspect = singlePlayer ? CARD_SINGLE_ASPECT : CARD_MULTI_ASPECT;

        // Cover the pane: crop whichever axis overflows so the art keeps its proportions.
        float paneAspect = previewW / previewH;
        float visibleU = paneAspect >= artAspect ? 1f : artAspect / paneAspect;
        float visibleV = paneAspect >= artAspect ? paneAspect / artAspect : 1f;
        visibleU = 1f / Math.max(1f, visibleU);
        visibleV = 1f / Math.max(1f, visibleV);

        RenderUtil.TEXTURE_RECT.draw(matrixStack, previewX, previewY, previewW, previewH, previewRadius,
                ColorUtil.setAlpha(Color.WHITE, hovered ? 255 : 225),
                (1f - visibleU) / 2f, (1f - visibleV) / 2f, visibleU, visibleV, art);

        outline(matrixStack, previewX, previewY, previewW, previewH, previewRadius,
                ColorUtil.setAlpha(AMBER_300, 90), d(1f));
    }

    // ---------------------------------------------------------------- pills

    /** Small translucent button with a leading icon, as in the design's settings row. */
    public void renderPill(PoseStack matrixStack, AbstractWidget widget, Role role) {
        float x = widget.getX();
        float y = widget.getY();
        float w = widget.getWidth();
        float h = widget.getHeight();

        boolean hovered = widget.isHoveredOrFocused();
        float radius = d(8f);

        glass(matrixStack, x, y, w, h, radius, hovered
                ? ColorUtil.setAlpha(AMBER_300, 210)
                : ColorUtil.setAlpha(new Color(10, 13, 24), 210), 0.30f, 0.7f);

        outline(matrixStack, x, y, w, h, radius,
                ColorUtil.setAlpha(AMBER_300, hovered ? 155 : 82), d(1f));

        String glyph = (role == Role.PILL_SETTINGS ? Icons.OPTIONS : Icons.FOLDER).getLetter();
        float fontSize = d(13f);
        float glyphSize = d(14f);
        // Some language packs ship the label with a trailing reset code; drawn through a font
        // that knows nothing about formatting it would show up as literal "§r".
        String label = strip(widget.getMessage().getString());

        float glyphWidth = Fonts.ICONS.getWidth(glyph, glyphSize);
        float labelWidth = Fonts.PS_BOLD.getWidth(label, fontSize);
        float contentX = x + (w - (glyphWidth + d(8f) + labelWidth)) / 2f;

        Fonts.ICONS.drawText(matrixStack, glyph, contentX, y + (h - glyphSize) / 2f, glyphSize, Color.WHITE);
        Fonts.PS_BOLD.drawText(matrixStack, label, contentX + glyphWidth + d(8f),
                y + (h - fontSize) / 2f, fontSize, Color.WHITE);
    }

    // ---------------------------------------------------------------- exit pill

    public float exitWidth()  { return d(196f); }
    public float exitHeight() { return d(60f); }
    public float exitX()      { return d(32f); }

    public float exitY() {
        return Minecraft.getInstance().getWindow().getGuiScaledHeight() - exitHeight() - d(32f);
    }

    /**
     * Bottom-left "hold to exit" chip. It is not a widget: the design asks for a press-and-hold,
     * which a vanilla button cannot express, so hover and hold are tracked here directly.
     */
    public void renderExit(PoseStack matrixStack, double mouseX, double mouseY) {
        float x = exitX();
        float y = exitY();
        float w = exitWidth();
        float h = exitHeight();
        float radius = d(12f);

        boolean hovered = mouseX >= x && mouseX <= x + w && mouseY >= y && mouseY <= y + h;
        float progress = holdProgress(hovered);

        glass(matrixStack, x, y, w, h, radius, ColorUtil.setAlpha(Color.WHITE, 230), 0.42f, 0.75f);

        // Fill sweeps across while the button is held, then the game quits.
        if (progress > 0f) {
            RenderUtil.RECT.draw(matrixStack, x, y, w * progress, h, radius,
                    ColorUtil.setAlpha(AMBER_300, 150));
        }

        float pad = d(8f);
        float tile = d(44f);
        RenderUtil.RECT.draw(matrixStack, x + pad, y + (h - tile) / 2f, tile, tile, d(8f), INK_900);

        String glyph = Icons.QUIT.getLetter();
        float glyphSize = tile * 0.42f;
        float glyphWidth = Fonts.ICONS.getWidth(glyph, glyphSize);
        Fonts.ICONS.drawText(matrixStack, glyph,
                x + pad + (tile - glyphWidth) / 2f,
                y + (h - glyphSize) / 2f, glyphSize, AMBER_300);

        float textX = x + pad + tile + d(10f);
        float hintSize = d(11f);
        float labelSize = d(13f);
        float blockHeight = hintSize + d(2f) + labelSize;
        float textY = y + (h - blockHeight) / 2f;

        Fonts.PS_BOLD.drawText(matrixStack, "HOLD", textX, textY, hintSize, AMBER_700);

        float labelY = textY + hintSize + d(2f);
        Fonts.PS_BOLD.drawText(matrixStack, "Slide to exit", textX, labelY, labelSize, INK_900);

        // Nudging chevron, matching the design's looping hint animation.
        float nudge = (float) Math.sin(System.currentTimeMillis() / 220.0) * d(3f);
        String chevron = Icons.RIGHTR.getLetter();
        float chevronSize = d(12f);
        Fonts.ICONS.drawText(matrixStack, chevron,
                textX + Fonts.PS_BOLD.getWidth("Slide to exit", labelSize) + d(6f) + nudge,
                labelY + (labelSize - chevronSize) / 2f, chevronSize, AMBER_500);
    }

    /** 0..1 while the left button is held over the chip; quits the game once it completes. */
    private float holdProgress(boolean hovered) {
        Minecraft mc = Minecraft.getInstance();
        Window window = mc.getWindow();
        boolean pressed = GLFW.glfwGetMouseButton(window.handle(), GLFW.GLFW_MOUSE_BUTTON_LEFT) == GLFW.GLFW_PRESS;

        if (!hovered || !pressed) {
            exitHoldStart = 0L;
            return 0f;
        }

        long now = System.currentTimeMillis();
        if (exitHoldStart == 0L) {
            exitHoldStart = now;
            return 0f;
        }

        float progress = (now - exitHoldStart) / 900f;
        if (progress >= 1f) {
            exitHoldStart = 0L;
            mc.stop();
            return 1f;
        }
        return progress;
    }

    // ---------------------------------------------------------------- helpers

    /** Outline drawn as four bands so it works on any rounded rect. */
    private void outline(PoseStack matrixStack, float x, float y, float w, float h, float round, Color color, float thickness) {
        float t = Math.max(1f, thickness);
        RenderUtil.RECT.draw(matrixStack, x + round, y, w - round * 2f, t, 0f, color);
        RenderUtil.RECT.draw(matrixStack, x + round, y + h - t, w - round * 2f, t, 0f, color);
        RenderUtil.RECT.draw(matrixStack, x, y + round, t, h - round * 2f, 0f, color);
        RenderUtil.RECT.draw(matrixStack, x + w - t, y + round, t, h - round * 2f, 0f, color);
    }

    /** Drops Minecraft formatting codes from a label. */
    public String strip(String text) {
        return text == null ? "" : text.replaceAll("§.", "");
    }
}
