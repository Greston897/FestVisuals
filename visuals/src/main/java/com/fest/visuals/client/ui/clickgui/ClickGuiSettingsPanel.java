package com.fest.visuals.client.ui.clickgui;

import java.awt.Color;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.util.Mth;
import lombok.Getter;

import com.fest.visuals.api.utils.animation.AnimationUtil;
import com.fest.visuals.api.utils.animation.Easing;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.math.MouseUtil;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.ScissorUtil;
import com.fest.visuals.api.utils.render.fonts.Fonts;
import com.fest.visuals.client.ui.clickgui.module.ModuleComponent;
import com.fest.visuals.client.ui.clickgui.module.SettingComponent;
import com.fest.visuals.client.ui.clickgui.module.settings.CanvasComponent;
import com.fest.visuals.client.ui.clickgui.module.settings.StringComponent;

/**
 * Floating card holding one module's settings.
 *
 * <p>Cards live outside the main panel and can be dragged anywhere, so several modules can be
 * tuned side by side. A card grows out of the row that spawned it вЂ” the open animation scales the
 * whole card around that point вЂ” and shrinks back into nothing when closed.
 */
public class ClickGuiSettingsPanel {
    @Getter private final ModuleComponent component;

    private final AnimationUtil openAnimation = new AnimationUtil();
    private final float originX;
    private final float originY;

    private float x;
    private float y;
    private float height;
    private float scroll;
    private final AnimationUtil scrollAnimation = new AnimationUtil();

    private boolean closing;
    private boolean dragging;
    private float dragX;
    private float dragY;

    // Geometry of the last frame the card actually drew, so hit testing runs against what the
    // player sees rather than against where the card would be if it were not animating.
    private float drawScale = 1f;
    private float pivotX;
    private float pivotY;
    private float slideX;
    private float slideY;
    private boolean drawn;

    public ClickGuiSettingsPanel(ModuleComponent component, float x, float y, float originX, float originY) {
        this.component = component;
        this.x = x;
        this.y = y;
        this.originX = originX - x;
        this.originY = originY - y;
        openAnimation.setValue(0.0);
    }

    public boolean isClosing() { return closing; }
    public void close() { closing = true; }

    public boolean isGone() {
        return closing && openAnimation.getValue() <= 0.02;
    }

    public void render(GuiGraphicsExtractor context, int mouseX, int mouseY, float delta, float alpha) {
        PoseStack matrices = RenderUtil.matrices();

        openAnimation.update();
        openAnimation.run(closing ? 0.0 : 1.0, closing ? 220 : 460, closing ? Easing.CUBIC_IN : Easing.EXPO_OUT, true);
        float open = (float) openAnimation.getValue();
        if (open <= 0.01f) return;

        float width = cardWidth();
        float measured = measure();
        this.height = Math.min(measured, ClickGuiLayout.panelMaxHeight());

        if (dragging) {
            x = (float) mapX(mouseX) - dragX;
            y = (float) mapY(mouseY) - dragY;
        }
        clampToScreen();

        // Scale about the card's own centre and slide in from the row that opened it. Scaling
        // about the distant row instead would throw the card tens of pixels off its own
        // coordinates while the animation runs, and every click would miss.
        drawScale = 0.9f + 0.1f * open;
        pivotX = x + width / 2f;
        pivotY = y + height / 2f;
        slideX = originX * (1f - open) * 0.25f;
        slideY = originY * (1f - open) * 0.25f;
        drawn = true;

        matrices.pushPose();
        matrices.translate(slideX, slideY, 0f);
        matrices.translate(pivotX, pivotY, 0f);
        matrices.scale(drawScale, drawScale, 1f);
        matrices.translate(-pivotX, -pivotY, 0f);

        float panelAlpha = alpha * open;
        int full = (int) (panelAlpha * 255f);
        float round = ClickGuiLayout.panelRound();

        RenderUtil.BLUR_RECT.draw(matrices, x, y, width, height, round,
                ColorUtil.setAlpha(UIColors.blur(), full), 0.24f);

        Color tint = ColorUtil.setAlpha(UIColors.primary(), (int) (panelAlpha * 26f));
        Color clear = ColorUtil.setAlpha(UIColors.primary(), 0);
        RenderUtil.GRADIENT_RECT.draw(matrices, x, y, width, height, round, tint, clear, clear, clear);

        float headerH = ClickGuiLayout.scaled(24f);
        float titleSize = ClickGuiLayout.scaled(7.6f);
        Fonts.PS_BOLD.drawText(matrices, component.getModule().getName(), x + ClickGuiLayout.scaled(10f),
                y + (headerH - titleSize) / 2f, titleSize, UIColors.textColor(full));

        boolean overClose = hoveredClose(mapX(mouseX), mapY(mouseY));
        float closeCx = x + width - closeInset();
        float closeCy = y + headerH / 2f;
        if (overClose) {
            float size = closeSize();
            RenderUtil.RECT.draw(matrices, closeCx - size / 2f, closeCy - size / 2f, size, size, size / 2f,
                    ColorUtil.setAlpha(UIColors.negativeColor(), (int) (panelAlpha * 45f)));
        }
        Color closeColor = overClose
                ? ColorUtil.setAlpha(UIColors.negativeColor(), full)
                : UIColors.inactiveTextColor((int) (full * 0.75f));
        ClickGuiIcons.cross(matrices, closeCx, closeCy, ClickGuiLayout.scaled(7f), closeColor);

        renderBody(context, matrices, mouseX, mouseY, delta, panelAlpha, width, headerH, measured);

        matrices.popPose();
    }

    private void renderBody(GuiGraphicsExtractor context, PoseStack matrices, int mouseX, int mouseY, float delta,
                            float panelAlpha, float width, float headerH, float measured) {
        float bodyY = y + headerH;
        float bodyH = height - headerH - ClickGuiLayout.scaled(8f);

        scrollAnimation.update();
        scrollAnimation.run(scroll, 300, Easing.EXPO_OUT);
        float offset = (float) scrollAnimation.getValue();

        ScissorUtil.start(matrices, x, bodyY, width, bodyH);

        float rowY = bodyY + offset;
        float rowX = x + ClickGuiLayout.scaled(10f);
        float rowW = width - ClickGuiLayout.scaled(20f);
        int index = 0;

        int localMouseX = (int) mapX(mouseX);
        int localMouseY = (int) mapY(mouseY);

        for (SettingComponent setting : component.getSettings()) {
            setting.getVisibleAnimation().update();
            setting.getVisibleAnimation().run(setting.getSetting().isVisible() ? 1.0 : 0.0, 200, Easing.EXPO_OUT);
            float visible = (float) setting.getVisibleAnimation().getValue();
            if (visible <= 0.01f) continue;

            // Settings cascade in one after another while the card opens.
            float stagger = Mth.clamp(((float) openAnimation.getValue() * 1.6f) - index * 0.13f, 0f, 1f);
            float ease = Easing.EXPO_OUT.apply(stagger);

            setting.setX(rowX + ClickGuiLayout.scaled(10f) * (1f - ease));
            setting.setY(rowY);
            setting.setWidth(rowW);
            setting.setAlpha(visible * panelAlpha * ease);
            setting.render(context, localMouseX, localMouseY, delta);

            rowY += (setting.getHeight() + ClickGuiLayout.scaled(4f)) * visible;
            index++;
        }

        ScissorUtil.stop(matrices);

        float overflow = measured - height;
        if (overflow > 0f) {
            scroll = Mth.clamp(scroll, -overflow, 0f);

            // Scroll indicator riding the right edge.
            float trackH = bodyH;
            float thumbH = Math.max(ClickGuiLayout.scaled(14f), trackH * (bodyH / measured));
            float progress = overflow == 0f ? 0f : -offset / overflow;
            float thumbY = bodyY + (trackH - thumbH) * Mth.clamp(progress, 0f, 1f);
            RenderUtil.RECT.draw(matrices, x + width - ClickGuiLayout.scaled(3.5f), thumbY, ClickGuiLayout.scaled(1.6f), thumbH,
                    ClickGuiLayout.scaled(0.8f), ColorUtil.setAlpha(UIColors.primary(), (int) (panelAlpha * 120f)));
        } else {
            scroll = 0f;
        }
    }

    private float measure() {
        float total = ClickGuiLayout.scaled(24f) + ClickGuiLayout.scaled(8f);
        for (SettingComponent setting : component.getSettings()) {
            if (!setting.getSetting().isVisible()) continue;
            total += setting.getHeight() + ClickGuiLayout.scaled(4f);
        }
        return total;
    }

    /**
     * Screen coordinates to card coordinates: undoes the slide and the scale of the open
     * animation. Everything below hit-tests in card space, the same space the card draws in.
     */
    private double mapX(double mouseX) {
        return (mouseX - slideX - pivotX) / drawScale + pivotX;
    }

    private double mapY(double mouseY) {
        return (mouseY - slideY - pivotY) / drawScale + pivotY;
    }

    private void clampToScreen() {
        float screenW = Minecraft.getInstance().getWindow().getGuiScaledWidth();
        float screenH = Minecraft.getInstance().getWindow().getGuiScaledHeight();
        float margin = ClickGuiLayout.scaled(4f);
        x = Mth.clamp(x, margin - cardWidth() * 0.5f, screenW - cardWidth() * 0.5f);
        y = Mth.clamp(y, margin, Math.max(margin, screenH - ClickGuiLayout.scaled(20f)));
    }

    /**
     * Cards are one width, except when they hold a paint grid: seventeen cells across the normal
     * card leaves each one a few pixels wide, which is not something you can draw on.
     */
    private float cardWidth() {
        for (SettingComponent setting : component.getSettings()) {
            if (setting instanceof CanvasComponent) return ClickGuiLayout.panelWidth() * 1.35f;
        }
        return ClickGuiLayout.panelWidth();
    }

    public boolean hovered(double mouseX, double mouseY) {
        return drawn && MouseUtil.isHovered(mapX(mouseX), mapY(mouseY), x, y, cardWidth(), height);
    }

    private float closeInset() {
        return ClickGuiLayout.scaled(12f);
    }

    private float closeSize() {
        return ClickGuiLayout.scaled(18f);
    }

    /** Centred on the drawn cross, and generous вЂ” a 7px glyph is a hard thing to hit. */
    private boolean hoveredClose(double mouseX, double mouseY) {
        float size = closeSize();
        float cx = x + cardWidth() - closeInset();
        float cy = y + ClickGuiLayout.scaled(24f) / 2f;
        return MouseUtil.isHovered(mouseX, mouseY, cx - size / 2f, cy - size / 2f, size, size);
    }

    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        if (!hovered(mouseX, mouseY)) return false;

        double localX = mapX(mouseX);
        double localY = mapY(mouseY);

        if (button == 0 && hoveredClose(localX, localY)) {
            close();
            return true;
        }

        // Left-drag by the title bar, right-drag from anywhere вЂ” settings rows fill most of a
        // card, so the header alone is a thin target once a module has many of them.
        float headerH = ClickGuiLayout.scaled(24f);
        boolean onHeader = MouseUtil.isHovered(localX, localY, x, y, cardWidth(), headerH);
        if ((button == 0 && onHeader) || button == 1) {
            dragging = true;
            dragX = (float) localX - x;
            dragY = (float) localY - y;
            return true;
        }

        for (SettingComponent setting : component.getSettings()) {
            if (setting.getAlpha() < 0.4f) continue;
            setting.mouseClicked(localX, localY, button);
        }
        return true;
    }

    public void mouseReleased(double mouseX, double mouseY, int button) {
        dragging = false;
        for (SettingComponent setting : component.getSettings()) {
            setting.mouseReleased(mapX(mouseX), mapY(mouseY), button);
        }
    }

    public boolean mouseScrolled(double mouseX, double mouseY, double amount) {
        if (!hovered(mouseX, mouseY)) return false;
        scroll += (float) (amount * ClickGuiLayout.scaled(14f));
        return true;
    }

    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        boolean consumed = false;
        for (SettingComponent setting : component.getSettings()) {
            if (setting.getAlpha() < 0.4f) continue;
            // A focused text field owns the key — Escape has to leave the field before it is
            // allowed to close the card or the whole GUI.
            if (setting instanceof StringComponent field && field.isFocused()) consumed = true;
            setting.keyPressed(keyCode, scanCode, modifiers);
        }
        return consumed;
    }

    public boolean charTyped(char chr) {
        boolean consumed = false;
        for (SettingComponent setting : component.getSettings()) {
            if (setting.getAlpha() < 0.4f) continue;
            if (setting instanceof StringComponent field && field.isFocused()) consumed = true;
            setting.charTyped(chr);
        }
        return consumed;
    }
}
