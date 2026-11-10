package com.fest.visuals.client.ui.widget.overlay;

import com.mojang.blaze3d.vertex.PoseStack;

import com.fest.visuals.api.system.draggable.Draggable;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.client.features.modules.hud.ScoreboardHudModule;
import com.fest.visuals.client.ui.widget.Widget;

/**
 * Handle for the vanilla scoreboard: it carries the draggable position and paints the background.
 * The scoreboard itself is drawn by the game (see MixinInGameHud), shifted by the difference
 * between this widget's position and where vanilla would put it.
 */
public class ScoreboardWidget extends Widget {
    /** Where vanilla drew the sidebar last frame, before any shift, in GUI coordinates. */
    private static float vanillaX, vanillaY, vanillaWidth, vanillaHeight;
    private static long reportedAt;
    private static boolean placed;

    public ScoreboardWidget() {
        super(300f, 60f);
    }

    @Override
    public String getName() { return "Scoreboard"; }

    /** Called by the mixin after vanilla finished drawing. */
    public static void report(float x, float y, float width, float height) {
        vanillaX = x;
        vanillaY = y;
        vanillaWidth = width;
        vanillaHeight = height;
        reportedAt = width > 0f && height > 0f ? System.currentTimeMillis() : 0L;
    }

    /** The sidebar counts as shown only if vanilla drew it very recently (no objective: not drawn). */
    private static boolean visible() {
        return reportedAt != 0L && System.currentTimeMillis() - reportedAt < 250L;
    }

    /** How far the vanilla drawing must move to land on the widget; zero until first placed. */
    public static float[] shift(Draggable draggable) {
        if (!visible() || !placed) return new float[] {0f, 0f};
        return new float[] {draggable.getX() - vanillaX, draggable.getY() - vanillaY};
    }

    @Override
    public void render(PoseStack matrixStack) {
        Draggable draggable = getDraggable();
        if (!visible()) {
            draggable.setWidth(0f);
            draggable.setHeight(0f);
            return;
        }

        if (!placed) {
            // First sighting: start exactly where vanilla draws it.
            draggable.setX(vanillaX);
            draggable.setY(vanillaY);
            placed = true;
        }

        draggable.setWidth(vanillaWidth);
        draggable.setHeight(vanillaHeight);

        if (ScoreboardHudModule.getInstance().background.getValue()) {
            float x = draggable.getX(), y = draggable.getY(), w = vanillaWidth, h = vanillaHeight;
            FestRenderer.withBackdrop(() -> RenderUtil.BLUR_RECT.draw(matrixStack, x, y, w, h, getGap() * 2f, UIColors.widgetBlur(), 0.22f));
        }
    }
}
