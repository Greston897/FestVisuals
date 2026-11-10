package com.fest.visuals.inject.render;

import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.client.ui.menu.MainMenuTheme;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(GameRenderer.class)
public class MixinFestRendererFlush {
    @Inject(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/render/GuiRenderer;render()V"))
    private void festvisuals$flushBackdrop(DeltaTracker deltaTracker, boolean renderLevel, CallbackInfo ci) {
        // The blur targets have to be rebuilt before backdrops sample them. Skipped on loading
        // screens: compiling the blur pipeline before the resource reload installs our shader
        // sources caches it as invalid for the whole session.
        Minecraft mc = Minecraft.getInstance();
        if (mc.level != null && mc.player != null) {
            com.fest.visuals.api.utils.render.KawaseBlurProgram.render(com.fest.visuals.api.utils.render.RenderUtil.matrices());
        } else if (festvisuals$menuBlur(mc)) {
            // Title, alt manager and the other out-of-world screens share one cached wallpaper blur.
            MainMenuTheme.prepareGlassBlur();
        }
        FestRenderer.flushBackdrop();
    }

    /** Loading overlays must not compile the blur pipeline before the shader sources are in. */
    private boolean festvisuals$menuBlur(Minecraft mc) {
        Screen screen = mc.gui.screen();
        if (screen == null) return false;
        String name = screen.getClass().getSimpleName();
        return !name.contains("Loading") && !name.contains("Overlay");
    }

    @Inject(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/render/GuiRenderer;endFrame()V", shift = At.Shift.AFTER))
    private void festvisuals$flushUI(DeltaTracker deltaTracker, boolean renderLevel, CallbackInfo ci) {
        FestRenderer.getInstance().flush();
    }
}
