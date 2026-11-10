package com.fest.visuals.client.ui.clickgui;

import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.client.services.RenderService;

/**
 * Metrics for the click GUI, expressed in design units and scaled by the interface scale.
 *
 * <p>The window is one panel: a top bar carrying the search field and the tab strip, a large
 * category title beneath it, and the content area. Settings live in floating cards outside the
 * panel, so nothing here reserves room for them.
 */
public final class ClickGuiLayout {
    private ClickGuiLayout() {}

    /**
     * The whole GUI is drawn smaller than the rest of the interface вЂ” the design is dense, and at
     * full interface scale the panel swallowed the screen.
     */
    private static final float COMPACT = 0.66f;

    public static float scaled(float value) {
        return RenderService.getInstance().scaled(value * COMPACT);
    }

    /** Theme colour at the requested fade. Panels use it as a veil over liquid glass, not a solid fill. */
    public static java.awt.Color body(float alpha) {
        return ColorUtil.setAlpha(UIColors.blur(), (int) (alpha * 255f));
    }

    public static float windowWidth() {
        return scaled(470f);
    }

    public static float windowHeight() {
        return scaled(292f);
    }

    /** Outer corner radius of the panel. */
    public static float round() {
        return scaled(16f);
    }

    /** Horizontal breathing room between the panel edge and its content. */
    public static float pad() {
        return scaled(18f);
    }

    public static float topBarHeight() {
        return scaled(40f);
    }

    /** The big "вљЎ Render" line under the tab strip. */
    public static float titleHeight() {
        return scaled(38f);
    }

    /** Everything above the scrollable content вЂ” kept for callers that place their own body. */
    public static float headerHeight() {
        return topBarHeight() + titleHeight();
    }

    public static float rowHeight() {
        return scaled(32f);
    }

    public static float columnGap() {
        return scaled(22f);
    }

    public static float columnWidth() {
        return (windowWidth() - pad() * 2f - columnGap()) / 2f;
    }

    public static float contentX(float windowX) {
        return windowX;
    }

    public static float contentY(float windowY) {
        return windowY + headerHeight();
    }

    public static float contentWidth() {
        return windowWidth();
    }

    public static float contentHeight() {
        return windowHeight() - headerHeight();
    }

    /** Floating settings cards. */
    public static float panelWidth() {
        return scaled(136f);
    }

    public static float panelRound() {
        return scaled(12f);
    }

    public static float panelMaxHeight() {
        return scaled(230f);
    }
}
