package com.fest.visuals.client.ui.widget.overlay;

import com.fest.visuals.api.utils.animation.AnimationUtil;
import com.fest.visuals.api.utils.color.ColorUtil;
import com.fest.visuals.api.utils.color.UIColors;
import com.fest.visuals.api.utils.combat.CombatTracker;
import com.fest.visuals.api.utils.math.MathUtil;
import com.fest.visuals.api.utils.render.RenderUtil;
import com.fest.visuals.client.features.modules.hud.TargetHudModule;
import com.fest.visuals.client.ui.widget.Widget;
import com.mojang.blaze3d.vertex.PoseStack;
import java.awt.*;
import java.time.Duration;
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.phys.EntityHitResult;

public class TargetInfoWidget extends Widget {
    @Override
    public String getName() {
        return "Target info";
    }

    public TargetInfoWidget() {
        super(30f, 30f);
    }

    private final AnimationUtil showAnimation = new AnimationUtil();

    private float healthAnimation = 0f;
    /** The chunk of health just lost, lingering behind the bar before it drains away. */
    private float ghostHealth = 0f;
    private long ghostHoldUntil;

    private LivingEntity target;

    @Override
    public void render(PoseStack matrixStack) {
        update();
        LivingEntity pretendTarget = getTarget();

        if (pretendTarget != null) {
            target = pretendTarget;
        }

        if (showAnimation.getValue() <= 0.0 || target == null) return;

        
        float actualHealth = target instanceof Player p ? getFunTimeHealth(p) : target.getHealth();
        float maxHealth = (target instanceof Player && TargetHudModule.getInstance().ftMode.getValue()) ? Math.max(20f, actualHealth) : target.getMaxHealth();
        float healthRatio = Mth.clamp(actualHealth / maxHealth, 0f, 1f);

        healthAnimation = Mth.clamp(MathUtil.interpolate(healthAnimation, healthRatio, 0.3f), 0f, 1f);

        long now = System.currentTimeMillis();
        if (healthRatio > ghostHealth) {
            ghostHealth = healthRatio;
        } else if (healthRatio < ghostHealth - 0.001f && ghostHoldUntil == 0) {
            ghostHoldUntil = now + 350;
        }
        if (ghostHoldUntil != 0 && now >= ghostHoldUntil) {
            ghostHealth = MathUtil.interpolate(ghostHealth, healthRatio, 0.12f);
            if (ghostHealth - healthRatio < 0.002f) {
                ghostHealth = healthRatio;
                ghostHoldUntil = 0;
            }
        }
        float x = getDraggable().getX();
        float y = getDraggable().getY();

        float anim = (float) showAnimation.getValue();

        float[] headProperties = headProperties(x, y);
        float headX = headProperties[0];
        float headY = headProperties[1];
        float headSize = headProperties[2];

        float bigFontSize = headSize * 0.35f;
        float smallFontSize = (headSize * 0.4f) * 0.7f;

        String targetName = target.getName().getString();
        
        String healthText = String.format("%.1f", actualHealth + (TargetHudModule.getInstance().ftMode.getValue() ? 0 : target.getAbsorptionAmount())) + "HP";

        float healthTextWidth = getMediumFont().getWidth(healthText, smallFontSize);

        float offset = getGap() * 3f;
        float margin = getGap() * 2f;
        float width = headSize * 3.7f + margin * 2f;
        float height = headSize + getGap() * 2f;
        float backgroundRound = offset * 0.7f;

        int fullAlpha = (int) (anim * 255f);

        float[] healthBarProperties = healthBarProperties();
        float healthBarHeight = healthBarProperties[0];
        float healthBarRound = healthBarProperties[1];
        float healthBarY = y + height - healthBarHeight - margin;
        float healthBarX = x + headSize + margin;
        float healthBarWidth = width - margin * 2.5f - headSize - healthTextWidth;

        float diffHealth = Math.abs(smallFontSize - healthBarHeight) / 2f;

        float nameDiffToHealthBar = Math.abs((y + margin) - (healthBarY - margin / 2f));
        float nameY = y + margin + nameDiffToHealthBar / 2f - bigFontSize / 2f;

        // Pops in from slightly smaller, and jolts sideways for a moment when the target is hit.
        float pop = 0.85f + 0.15f * anim;
        float jolt = target.hurtTime > 0 ? (float) Math.sin(target.hurtTime * 2.4) * target.hurtTime * 0.25f : 0f;
        matrixStack.pushPose();
        matrixStack.translate(x + width / 2f + jolt, y + height / 2f, 0f);
        matrixStack.scale(pop, pop, 1f);
        matrixStack.translate(-(x + width / 2f), -(y + height / 2f), 0f);

        RenderUtil.BLUR_RECT.draw(matrixStack, x, y, width, height, backgroundRound, UIColors.widgetBlur(fullAlpha), 0.22f);

        Color textColor = UIColors.textColor(fullAlpha);
        getMediumFont().drawWrap(matrixStack, targetName, healthBarX, nameY, width - headSize - margin, bigFontSize, textColor, scaled(9f), Duration.ofMillis(2500), Duration.ofMillis(1700));
        getMediumFont().drawText(matrixStack, healthText, x + width - healthTextWidth - margin, healthBarY - diffHealth, smallFontSize, textColor);

        RenderUtil.RECT.draw(matrixStack, healthBarX, healthBarY, healthBarWidth, healthBarHeight, healthBarRound, UIColors.backgroundBlur(fullAlpha));

        if (ghostHealth > healthAnimation) {
            Color ghost = ColorUtil.setAlpha(new Color(255, 235, 235), (int) (fullAlpha * 0.8f));
            RenderUtil.RECT.draw(matrixStack, healthBarX, healthBarY, healthBarWidth * ghostHealth, healthBarHeight, healthBarRound, ghost);
        }

        Color color1 = UIColors.gradient(0, fullAlpha);
        Color color2 = UIColors.gradient(90, fullAlpha);
        RenderUtil.GRADIENT_RECT.draw(matrixStack, healthBarX, healthBarY, healthBarWidth * healthAnimation, healthBarHeight, healthBarRound, color1, color2, color1, color2);

        if (target instanceof Player player) {
            float hurt = target.hurtTime / 10f;
            Color headColor = ColorUtil.setAlpha(ColorUtil.interpolate(new Color(255, 90, 90), Color.WHITE, hurt), fullAlpha);

            RenderUtil.TEXTURE_RECT.drawHead(matrixStack, player, headX, headY, headSize, headSize, getGap() / 2f, 0f, headColor);
        } else {
            float headFontSize = headSize * 0.8f;
            getSemiBoldFont().drawCenteredText(matrixStack, "?", headX + headSize / 2f, headY + headSize / 2f - headFontSize / 2f, headFontSize, UIColors.textColor(fullAlpha));
        }

        matrixStack.popPose();

        getDraggable().setWidth(width);
        getDraggable().setHeight(height);
    }

    private float[] healthBarProperties() {
        float height = scaled(5f);
        float round = height * 0.3f;

        return new float[]{height, round};
    }

    private float[] headProperties(float xPos, float yPos) {
        float x = xPos + getGap();
        float y = yPos + getGap();
        float size = scaled(25f);
        return new float[]{x, y, size};
    }

    
    private float getFunTimeHealth(Player target) {
        if (!TargetHudModule.getInstance().ftMode.getValue()) return target.getHealth();
        for (net.minecraft.world.entity.Entity e : mc.level.getEntitiesOfClass(net.minecraft.world.entity.Entity.class, target.getBoundingBox().inflate(2.0))) {
            if (e != target && e.hasCustomName()) {
                String name = e.getCustomName().getString().toLowerCase();
                if (name.contains("здоровья") || name.contains("hp") || name.contains("❤") || name.contains("\u2764")) {
                    String digits = name.replaceAll("[^0-9.]", "");
                    if (!digits.isEmpty()) {
                        try {
                            return Float.parseFloat(digits);
                        } catch (Exception ignored) {}
                    }
                }
            }
        }
        return target.getHealth();
    }

    private void update() {
        showAnimation.update();
        showAnimation.run(getTarget() != null ? 1.0 : 0.0, getDuration(), getEasing());
    }

    private LivingEntity getTarget() {
        // Preview target: while chat is open, show self so the HUD can be positioned.
        if (mc.gui.screen() instanceof ChatScreen) return mc.player;
        if (mc.player == null) return null;

        // Whoever was hit last, for a few seconds; otherwise whatever the crosshair is on.
        Entity recent = CombatTracker.getInstance().recentTarget(4000L);
        if (recent instanceof LivingEntity living && living.isAlive() && !living.isRemoved()
                && living.distanceTo(mc.player) < 16f) {
            return living;
        }
        if (mc.hitResult instanceof EntityHitResult hit && hit.getEntity() instanceof LivingEntity living && living.isAlive()) {
            return living;
        }
        return null;
    }

    @Override
    public float fontMul() { return TargetHudModule.getInstance().scale.getValue(); }
}
