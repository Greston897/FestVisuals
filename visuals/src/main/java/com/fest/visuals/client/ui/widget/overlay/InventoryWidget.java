package com.fest.visuals.client.ui.widget.overlay;

import com.mojang.blaze3d.vertex.PoseStack;

import java.awt.Color;

import com.fest.visuals.api.event.events.render.Render2DEvent;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.client.features.modules.hud.InventoryHudModule;
import com.fest.visuals.client.ui.widget.Widget;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.ItemStack;

/**
 * The main inventory (the 27 slots above the hotbar) as a 9x3 grid of item icons.
 *
 * <p>Slot plates and the panel go through the backdrop queue, because the icons themselves are
 * drawn by DrawContext during the vanilla GUI pass, which sits between the two renderer flushes.
 */
public class InventoryWidget extends Widget {
    private static final int COLUMNS = 9;
    private static final int ROWS = 3;

    /** Inventory indices 0-8 are the hotbar; the main grid starts right after it. */
    private static final int FIRST_SLOT = 9;

    public InventoryWidget() {
        super(20f, 140f);
    }

    @Override
    public String getName() { return "Inventory"; }

    private float slotSize() {
        return scaled(16f * InventoryHudModule.getInstance().scale.getValue());
    }

    @Override
    public void render(Render2DEvent.Render2DEventData event) {
        if (mc.player == null) return;

        PoseStack matrixStack = event.matrixStack();
        GuiGraphicsExtractor context = event.context();
        InventoryHudModule cfg = InventoryHudModule.getInstance();

        float slot = slotSize();
        float gap = scaled(2f);
        float pad = getGap() * 1.5f;
        boolean drawSlots = cfg.showSlots.getValue();

        float width = COLUMNS * slot + (COLUMNS - 1) * gap + pad * 2f;
        float height = ROWS * slot + (ROWS - 1) * gap + pad * 2f;

        float x = getDraggable().getX();
        float y = getDraggable().getY();

        getDraggable().setWidth(width);
        getDraggable().setHeight(height);

        FestRenderer.withBackdrop(() -> {
            RenderUtil.BLUR_RECT.draw(matrixStack, x, y, width, height, getGap() * 2f, UIColors.widgetBlur(), 0.22f);

            if (!drawSlots) return;
            for (int row = 0; row < ROWS; row++) {
                for (int column = 0; column < COLUMNS; column++) {
                    float slotX = x + pad + column * (slot + gap);
                    float slotY = y + pad + row * (slot + gap);
                    RenderUtil.RECT.draw(matrixStack, slotX, slotY, slot, slot, gap,
                            ColorUtil.setAlpha(Color.BLACK, 70));
                }
            }
        });

        // Whole-number scale and whole-pixel origins keep the sprites crisp instead of
        // resampling every item at a fractional size.
        int iconScale = Math.max(1, Math.round(slot / 16f));
        float iconSize = iconScale * 16f;
        float iconInset = (slot - iconSize) / 2f;
        for (int index = 0; index < COLUMNS * ROWS; index++) {
            ItemStack stack = mc.player.getInventory().getItem(FIRST_SLOT + index);
            if (stack == null || stack.isEmpty()) continue;

            float slotX = x + pad + (index % COLUMNS) * (slot + gap);
            float slotY = y + pad + (index / COLUMNS) * (slot + gap);

            context.pose().pushMatrix();
            context.pose().translate(Math.round(slotX + iconInset), Math.round(slotY + iconInset));
            context.pose().scale(iconScale, iconScale);
            context.item(stack, 0, 0);
            if (cfg.showCounts.getValue()) {
                context.itemDecorations(mc.font, stack, 0, 0);
            }
            context.pose().popMatrix();
        }
    }

    @Override
    public void render(PoseStack matrixStack) {}
}
