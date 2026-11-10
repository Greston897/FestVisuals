package com.fest.visuals.client.ui.widget.overlay;

import com.fest.visuals.api.event.events.render.Render2DEvent;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.client.features.modules.hud.ArmorHudModule;
import com.fest.visuals.client.ui.widget.Widget;
import com.mojang.blaze3d.vertex.PoseStack;

import java.awt.Color;
import java.util.ArrayList;
import java.util.List;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;

/** Equipped armour pieces with optional durability bars. Held items are deliberately excluded. */
public class ArmorWidget extends Widget {
    private static final EquipmentSlot[] ARMOR_SLOTS = { EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET };

    private final List<ItemStack> items = new ArrayList<>();

    public ArmorWidget() {
        super(400f, 200f);
    }

    @Override
    public String getName() {
        return "Armor";
    }

    private float itemSize() { return scaled(13f * ArmorHudModule.getInstance().scale.getValue()); }

    @Override
    public void render(Render2DEvent.Render2DEventData event) {
        PoseStack matrixStack = event.matrixStack();
        GuiGraphicsExtractor context = event.context();

        updateItems();
        if (items.isEmpty()) {
            getDraggable().setWidth(0f);
            getDraggable().setHeight(0f);
            return;
        }

        ArmorHudModule cfg = ArmorHudModule.getInstance();
        boolean vertical = cfg.isVertical();
        boolean durability = cfg.showDurability.getValue();

        float x = getDraggable().getX();
        float y = getDraggable().getY();

        float itemSize = itemSize();
        float gap = getGap();
        float barGap = scaled(1.5f);
        float barHeight = scaled(1.5f);

        // A cell is the icon plus, when enabled, the durability bar underneath it.
        float cellWidth = itemSize;
        float cellHeight = itemSize + (durability ? barGap + barHeight : 0f);
        float step = vertical ? cellHeight : cellWidth;
        float span = items.size() * step + (items.size() - 1) * gap;

        float width = (vertical ? cellWidth : span) + gap * 2f;
        float height = (vertical ? span : cellHeight) + gap * 2f;

        getDraggable().setWidth(width);
        getDraggable().setHeight(height);

        // The icons below go through DrawContext, which paints during the vanilla GUI pass —
        // after every normal widget is queued but before they are flushed. Routing the
        // background to the backdrop queue is what keeps it under the icons while still
        // sampling the same blur targets every other widget uses.
        FestRenderer.withBackdrop(() ->
                RenderUtil.BLUR_RECT.draw(matrixStack, x, y, width, height, gap * 2f, UIColors.widgetBlur(), 0.22f));

        float scaleFactor = itemSize / 16f;
        float cursorX = x + gap;
        float cursorY = y + gap;

        for (ItemStack item : items) {
            context.pose().pushMatrix();
            context.pose().translate(cursorX, cursorY);
            context.pose().scale(scaleFactor, scaleFactor);
            context.item(item, 0, 0);
            context.pose().popMatrix();

            // Queued after the flush, so bars land on top of the icons.
            if (durability) {
                drawDurability(context, item, cursorX, cursorY + itemSize + barGap, itemSize, barHeight);
            }

            if (vertical) cursorY += step + gap;
            else cursorX += step + gap;
        }
    }

    /**
     * Drawn through the vanilla GUI, in the same pass as the item icons. The client renderer flushes
     * after the whole GUI, which put these bars on top of any screen opened over the HUD.
     */
    private void drawDurability(GuiGraphicsExtractor context, ItemStack item, float x, float y, float width, float height) {
        int left = Math.round(x), top = Math.round(y);
        int right = Math.round(x + width), bottom = Math.round(y + Math.max(1f, height));
        context.fill(left, top, right, bottom, ColorUtil.setAlpha(Color.BLACK, 120).getRGB());

        if (!item.isDamageableItem() || item.getMaxDamage() <= 0) return;

        float remaining = 1f - (float) item.getDamageValue() / item.getMaxDamage();
        remaining = Math.max(0f, Math.min(1f, remaining));

        // interpolate(to, from, amount) returns `to` at 1 — full durability reads green.
        Color color = ColorUtil.interpolate(UIColors.positiveColor(), UIColors.negativeColor(), remaining);
        int filled = Math.round(x + width * remaining);
        if (filled > left) context.fill(left, top, filled, bottom, color.getRGB());
    }

    private void updateItems() {
        items.clear();
        if (mc.player == null) return;
        Player player = mc.player;

        for (EquipmentSlot slot : ARMOR_SLOTS) {
            ItemStack stack = player.getItemBySlot(slot);
            if (stack != null && !stack.isEmpty()) items.add(stack);
        }
    }

    @Override
    public void render(PoseStack matrixStack) {}
}
