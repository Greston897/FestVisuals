package com.fest.visuals.client.ui.clickgui;

import java.awt.Color;
import java.util.ArrayList;
import java.util.List;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.util.Mth;
import org.lwjgl.glfw.GLFW;

import com.fest.visuals.api.system.interfaces.QuickImports;
import com.fest.visuals.api.utils.animation.AnimationUtil;
import com.fest.visuals.api.utils.animation.Easing;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.math.MouseUtil;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.api.utils.render.ScissorUtil;
import com.fest.visuals.api.utils.render.fonts.Fonts;
import com.fest.visuals.client.features.modules.utility.SoundsModule;
import com.fest.visuals.client.ui.clickgui.module.ModuleComponent;
import com.fest.visuals.client.ui.theme.ThemeEditor;

/**
 * The click GUI: one panel with a search field, a tab strip, a large category title and a
 * two-column module grid, plus floating settings cards that live outside the panel.
 *
 * <p>Everything is animated off a handful of clocks. The window scales and fades on open and
 * close, the title swaps with a vertical wipe when the tab changes, the grid cascades its rows in,
 * and each card grows out of the row that spawned it.
 */
public class ScreenClickGUI extends Screen implements QuickImports {
    private static ScreenClickGUI instance;

    public static ScreenClickGUI getInstance() {
        if (instance == null) instance = new ScreenClickGUI();
        return instance;
    }

    private final AnimationUtil openAnimation = new AnimationUtil();
    private final AnimationUtil titleAnimation = new AnimationUtil();

    private final ClickGuiTopBar topBar = new ClickGuiTopBar();
    private final ClickGuiModuleList modules = new ClickGuiModuleList();
    private final ClickGuiConfigs configs = new ClickGuiConfigs();
    private final ClickGuiWaypoints waypointsUI = new ClickGuiWaypoints();
    private final ThemeEditor themeEditor = ThemeEditor.getInstance();

    private final List<ClickGuiSettingsPanel> panels = new ArrayList<>();

    private ClickGuiTab tab = ClickGuiTab.RENDER;
    private boolean closing;

    // The panel is movable, so its corner is state rather than something recomputed per frame.
    private float windowX;
    private float windowY;
    private boolean positioned;
    private boolean draggingWindow;
    private float windowDragX;
    private float windowDragY;

    public ScreenClickGUI() {
        super(Component.literal("FestVisuals GUI"));
        openAnimation.setValue(0.0);
        titleAnimation.setValue(1.0);
        modules.setOpener(this::openSettings);
    }

    @Override
    protected void init() {
        openAnimation.setValue(0.0);
        closing = false;
        draggingWindow = false;

        if (!positioned) {
            windowX = this.width / 2f - ClickGuiLayout.windowWidth() / 2f;
            windowY = this.height / 2f - ClickGuiLayout.windowHeight() / 2f;
            positioned = true;
        }
        clampWindow();

        modules.restart();
        SoundsModule.getInstance().playScreenSound(true);
        themeEditor.setOpen(tab == ClickGuiTab.THEME);
        themeEditor.setEmbedded(tab == ClickGuiTab.THEME);
    }

    @Override
    public void onClose() {
        configs.close();
        waypointsUI.close();
        themeEditor.setOpen(false);
        panels.clear();
    }

    private float openScale() {
        return 0.5f + 0.5f * (float) openAnimation.getValue();
    }

    /**
     * The whole window is drawn through a scale about the screen centre while it opens. Mouse
     * coordinates arrive in screen space, so they are mapped back through that scale before any
     * hit test runs -- otherwise clicks during the animation land somewhere else entirely.
     */
    private double mapX(double mouseX) {
        float centre = mc.getWindow().getGuiScaledWidth() / 2f;
        return (mouseX - centre) / openScale() + centre;
    }

    private double mapY(double mouseY) {
        float centre = mc.getWindow().getGuiScaledHeight() / 2f;
        return (mouseY - centre) / openScale() + centre;
    }

    /** Keeps the top bar reachable no matter where the panel was dragged. */
    private void clampWindow() {
        float screenW = mc.getWindow().getGuiScaledWidth();
        float screenH = mc.getWindow().getGuiScaledHeight();
        float margin = ClickGuiLayout.scaled(6f);
        windowX = Mth.clamp(windowX, margin - ClickGuiLayout.windowWidth() * 0.6f,
                screenW - ClickGuiLayout.windowWidth() * 0.4f);
        windowY = Mth.clamp(windowY, margin, Math.max(margin, screenH - ClickGuiLayout.topBarHeight() * 1.5f));
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor context, int mouseX, int mouseY, float delta) {
        super.extractRenderState(context, mouseX, mouseY, delta);

        if (themeEditor.isOpen() && tab != ClickGuiTab.THEME) switchTo(ClickGuiTab.THEME);

        openAnimation.update();
        openAnimation.run(closing ? 0.0 : 1.0, closing ? 240 : 520, closing ? Easing.CUBIC_IN : Easing.BACK_OUT, true);
        float open = (float) openAnimation.getValue();

        if (closing && open <= 0.01f) {
            mc.gui.setScreen(null);
            return;
        }

        // A previous frame may have died inside a scissor or a pushed pose; start clean either way.
        FestRenderer.getInstance().resetState();

        PoseStack matrices = RenderUtil.matrices();
        float screenW = mc.getWindow().getGuiScaledWidth();
        float screenH = mc.getWindow().getGuiScaledHeight();

        if (!positioned) {
            windowX = screenW / 2f - ClickGuiLayout.windowWidth() / 2f;
            windowY = screenH / 2f - ClickGuiLayout.windowHeight() / 2f;
            positioned = true;
        }

        int localMouseX = (int) mapX(mouseX);
        int localMouseY = (int) mapY(mouseY);

        if (draggingWindow) {
            windowX = localMouseX - windowDragX;
            windowY = localMouseY - windowDragY;
        }
        clampWindow();

        RenderUtil.RECT.draw(matrices, 0, 0, screenW, screenH, 0f, new Color(0, 0, 0, (int) (150 * net.minecraft.util.Mth.clamp(open, 0f, 1f))));

        // The whole window breathes in from slightly small; cards ride the same clock.
        float scale = 0.92f + 0.08f * open;
        matrices.pushPose();
        matrices.translate(screenW / 2f, screenH / 2f, 0f);
        matrices.scale(scale, scale, 1f);
        matrices.translate(-screenW / 2f, -screenH / 2f, 0f);

        renderWindow(context, matrices, windowX, windowY, open, localMouseX, localMouseY, delta);

        for (ClickGuiSettingsPanel panel : panels) {
            panel.render(context, localMouseX, localMouseY, delta, open);
        }
        panels.removeIf(ClickGuiSettingsPanel::isGone);

        matrices.popPose();
    }

    private void renderWindow(GuiGraphicsExtractor context, PoseStack matrices, float windowX, float windowY,
                              float open, int mouseX, int mouseY, float delta) {
        float width = ClickGuiLayout.windowWidth();
        float height = ClickGuiLayout.windowHeight();
        float round = ClickGuiLayout.round();
        int full = (int) (net.minecraft.util.Mth.clamp(open, 0f, 1f) * 255f);

        // Dark liquid glass. The Kawase frost shows through; the rim carries the pour.
        RenderUtil.BLUR_RECT.draw(matrices, windowX, windowY, width, height, round,
                new Color(4, 4, 6, full));

        // Top hairline, brightest in the middle, that draws itself outwards as the window opens.
        float edge = ClickGuiLayout.scaled(0.6f);
        float sweep = width * Easing.EXPO_OUT.apply(open);
        Color line = ColorUtil.setAlpha(Color.WHITE, (int) (net.minecraft.util.Mth.clamp(open, 0f, 1f) * 70f));
        Color fade = ColorUtil.setAlpha(Color.WHITE, 0);
        RenderUtil.GRADIENT_RECT.draw(matrices, windowX + (width - sweep) / 2f, windowY, sweep, edge, 0f, fade, fade, line, line);

        topBar.render(matrices, windowX, windowY, open, tab, mouseX, mouseY);
        renderTitle(matrices, windowX, windowY, open);

        if (tab == ClickGuiTab.THEME) {
            themeEditor.setAnim(open);
            themeEditor.setX(ClickGuiLayout.contentX(windowX) + ClickGuiLayout.pad());
            themeEditor.setY(ClickGuiLayout.contentY(windowY));
            themeEditor.setWidth(ClickGuiLayout.contentWidth() - ClickGuiLayout.pad() * 2f);
            themeEditor.setHeight(ClickGuiLayout.contentHeight() - ClickGuiLayout.scaled(10f));
            themeEditor.render(context, mouseX, mouseY, delta);
        } else if (tab == ClickGuiTab.CONFIGS && topBar.query().isEmpty()) {
            configs.render(context, windowX, windowY, open, mouseX, mouseY);
        } else if (tab == ClickGuiTab.WAYPOINTS && topBar.query().isEmpty()) {
            waypointsUI.render(context, windowX, windowY, open, mouseX, mouseY);
        } else {
            modules.render(context, windowX, windowY, open, tab, topBar.query(), mouseX, mouseY, delta);
        }
    }

    /** Big category line; on a tab change the new title wipes up into place behind a scissor. */
    private void renderTitle(PoseStack matrices, float windowX, float windowY, float open) {
        titleAnimation.update();
        titleAnimation.run(1.0, 420, Easing.BACK_OUT);
        float swap = (float) titleAnimation.getValue();

        float blockY = windowY + ClickGuiLayout.topBarHeight();
        float blockH = ClickGuiLayout.titleHeight();
        float iconSize = ClickGuiLayout.scaled(13f);
        float fontSize = ClickGuiLayout.scaled(14f);

        float x = windowX + ClickGuiLayout.pad();
        float rise = ClickGuiLayout.scaled(13f) * (1f - swap);
        int alpha = (int) (net.minecraft.util.Mth.clamp(open * swap, 0f, 1f) * 255f);

        ScissorUtil.start(matrices, windowX, blockY, ClickGuiLayout.windowWidth(), blockH);

        float centerY = blockY + blockH / 2f - ClickGuiLayout.scaled(3f) + rise;
        ClickGuiIcons.tab(matrices, tab, x, centerY - iconSize / 2f, iconSize,
                ColorUtil.setAlpha(UIColors.primary(), alpha), UIColors.blur((int) (net.minecraft.util.Mth.clamp(open, 0f, 1f) * 255f)));

        Fonts.PS_BOLD.drawText(matrices, tab.getLabel(), x + iconSize + ClickGuiLayout.scaled(8f),
                centerY - fontSize / 2f, fontSize, ColorUtil.setAlpha(Color.WHITE, alpha));

        String description = tab.getDescription();
        float descSize = ClickGuiLayout.scaled(6.4f);
        float descX = x + iconSize + ClickGuiLayout.scaled(8f)
                + Fonts.PS_BOLD.getWidth(tab.getLabel(), fontSize) + ClickGuiLayout.scaled(9f);
        Fonts.PS_MEDIUM.drawText(matrices, description, descX, centerY - descSize / 2f + ClickGuiLayout.scaled(1.5f),
                descSize, UIColors.inactiveTextColor((int) (alpha * 0.5f)));

        ScissorUtil.stop(matrices);
    }

    private void openSettings(ModuleComponent component, float rowX, float rowY) {
        for (ClickGuiSettingsPanel panel : panels) {
            if (panel.getComponent() == component && !panel.isClosing()) {
                panel.close();
                SoundsModule.getInstance().playClickSound(false);
                return;
            }
        }

        float screenW = mc.getWindow().getGuiScaledWidth();
        float screenH = mc.getWindow().getGuiScaledHeight();

        float gap = ClickGuiLayout.scaled(10f);
        // Cards sit to the right of the panel, sliding back over its edge only if the screen is
        // too narrow to hold them beside it.
        float right = windowX + ClickGuiLayout.windowWidth() + gap;
        float x = Math.min(right, screenW - ClickGuiLayout.panelWidth() - gap);

        // Cascade the cards so a second one does not land on top of the first.
        int alive = (int) panels.stream().filter(panel -> !panel.isClosing()).count();
        float y = windowY + ClickGuiLayout.scaled(8f) + alive * ClickGuiLayout.scaled(16f);
        y = Mth.clamp(y, gap, Math.max(gap, screenH - ClickGuiLayout.scaled(80f)));

        SoundsModule.getInstance().playClickSound(true);
        panels.add(new ClickGuiSettingsPanel(component, x, y, rowX, rowY + ClickGuiLayout.rowHeight() / 2f));
    }

    public void switchTo(ClickGuiTab next) {
        if (next == tab) {
            modules.restart();
            return;
        }

        tab = next;
        titleAnimation.setValue(0.0);
        modules.restart();
        topBar.resetSearchFocus();

        themeEditor.setOpen(next == ClickGuiTab.THEME);
        themeEditor.setEmbedded(next == ClickGuiTab.THEME);

        if (next == ClickGuiTab.CONFIGS) configs.refresh();
        else configs.close();
        if (next != ClickGuiTab.WAYPOINTS) waypointsUI.close();
    }

    @Override
    public boolean keyPressed(KeyEvent input) {
        // A focused text field takes priority over everything, Escape included.
        boolean typing = false;
        for (ClickGuiSettingsPanel panel : panels) {
            if (panel.keyPressed(input.key(), input.scancode(), input.modifiers())) typing = true;
        }
        if (typing) return true;
        if (tab == ClickGuiTab.WAYPOINTS && waypointsUI.keyPressed(input.key())) return true;
        if (tab == ClickGuiTab.CONFIGS && configs.keyPressed(input.key())) return true;

        // Escape otherwise gets out: it clears the search text if there is any, then closes the
        // cards, then the screen. Nothing else may swallow it.
        if (input.key() == GLFW.GLFW_KEY_ESCAPE) {
            if (!topBar.query().isEmpty()) {
                topBar.clearSearch();
                modules.restart();
                return true;
            }
            if (!panels.isEmpty()) {
                panels.forEach(ClickGuiSettingsPanel::close);
                return true;
            }
            closing = true;
            SoundsModule.getInstance().playScreenSound(false);
            return true;
        }

        if (topBar.keyPressed(input.key())) {
            modules.restart();
            return true;
        }

        if (tab == ClickGuiTab.THEME) {
            themeEditor.keyPressed(input.key(), input.scancode(), input.modifiers());
            return true;
        }

        modules.keyPressed(input.key(), input.scancode(), input.modifiers(), tab, topBar.query());
        return super.keyPressed(input);
    }

    @Override
    public boolean charTyped(CharacterEvent input) {
        char chr = (char) input.codepoint();
        // A focused waypoint field gets the text before the search box can take it.
        if (tab == ClickGuiTab.WAYPOINTS && waypointsUI.charTyped(chr)) return true;
        if (tab == ClickGuiTab.CONFIGS && configs.charTyped(chr)) return true;
        if (topBar.charTyped(chr)) {
            modules.restart();
            return true;
        }
        for (ClickGuiSettingsPanel panel : panels) {
            if (panel.charTyped(chr)) return true;
        }

        if (tab == ClickGuiTab.THEME) return themeEditor.charTyped(chr, 0);
        return super.charTyped(input);
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent click, boolean doubled) {
        double mx = mapX(click.x());
        double my = mapY(click.y());

        // Topmost card first, so overlapping cards resolve the way they are drawn.
        for (int i = panels.size() - 1; i >= 0; i--) {
            ClickGuiSettingsPanel panel = panels.get(i);
            if (panel.isClosing()) continue;
            if (panel.mouseClicked(mx, my, click.button())) {
                panels.add(panels.remove(i));
                return true;
            }
        }

        if (click.button() == 0 && topBar.clickedSearch(mx, my)) return true;

        ClickGuiTab hit = topBar.clickedTab(mx, my);
        if (hit != null) {
            switchTo(hit);
            return true;
        }

        // Empty space in the top bar is the drag handle for the whole panel.
        if (click.button() == 0 && MouseUtil.isHovered(mx, my, windowX, windowY,
                ClickGuiLayout.windowWidth(), ClickGuiLayout.topBarHeight())) {
            draggingWindow = true;
            windowDragX = (float) mx - windowX;
            windowDragY = (float) my - windowY;
            return true;
        }

        if (tab == ClickGuiTab.THEME) {
            themeEditor.mouseClicked(mx, my, click.button());
            return true;
        }
        if (tab == ClickGuiTab.CONFIGS && topBar.query().isEmpty()) {
            return configs.mouseClicked(mx, my, windowX, windowY) || super.mouseClicked(click, doubled);
        }
        if (tab == ClickGuiTab.WAYPOINTS && topBar.query().isEmpty()) {
            return waypointsUI.mouseClicked(mx, my, windowX, windowY) || super.mouseClicked(click, doubled);
        }

        modules.mouseClicked(mx, my, click.button(), windowX, windowY, tab, topBar.query());
        return super.mouseClicked(click, doubled);
    }

    @Override
    public boolean mouseReleased(MouseButtonEvent click) {
        double mx = mapX(click.x());
        double my = mapY(click.y());

        draggingWindow = false;
        for (ClickGuiSettingsPanel panel : panels) {
            panel.mouseReleased(mx, my, click.button());
        }
        if (tab == ClickGuiTab.THEME) themeEditor.mouseReleased(mx, my, click.button());
        else modules.mouseReleased(mx, my, click.button(), tab, topBar.query());
        return super.mouseReleased(click);
    }

    @Override
    public boolean mouseScrolled(double mouseX, double mouseY, double horizontalAmount, double verticalAmount) {
        double mx = mapX(mouseX);
        double my = mapY(mouseY);

        for (int i = panels.size() - 1; i >= 0; i--) {
            if (panels.get(i).mouseScrolled(mx, my, verticalAmount)) return true;
        }

        if (tab == ClickGuiTab.THEME) {
            themeEditor.mouseScrolled(mx, my, horizontalAmount, verticalAmount);
            return true;
        }
        if (tab == ClickGuiTab.CONFIGS && topBar.query().isEmpty()) {
            return configs.mouseScrolled(mx, my, verticalAmount, windowX, windowY);
        }
        if (tab == ClickGuiTab.WAYPOINTS && topBar.query().isEmpty()) {
            return waypointsUI.mouseScrolled(mx, my, verticalAmount, windowX, windowY);
        }
        return modules.mouseScrolled(mx, my, verticalAmount, windowX, windowY);
    }
}
