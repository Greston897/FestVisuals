package com.fest.visuals.api.utils.render;

import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.pipeline.TextureTarget;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.PoseStack;
import lombok.experimental.UtilityClass;
import com.fest.visuals.api.event.Listener;
import com.fest.visuals.api.event.events.other.FramebufferResizeEvent;
import com.fest.visuals.api.system.interfaces.QuickImports;
import com.fest.visuals.api.utils.render.pipeline.FestPipelines;
import com.fest.visuals.api.utils.render.pipeline.FestRenderer;
import com.fest.visuals.api.utils.render.pipeline.FestTextures;
import com.fest.visuals.api.utils.render.pipeline.FestUniform;

import java.util.ArrayList;
import java.util.List;

@UtilityClass
public class KawaseBlurProgram implements QuickImports {
    public final List<TextureTarget> fbos = new ArrayList<>();

    private boolean init = false;
    private long lastBlur;
    private boolean menuReady;
    private int menuW;
    private int menuH;
    private TextureTarget menuSource;

    public void load() {
        if (!init) {
            recreate();
            init = true;
        }

        FramebufferResizeEvent.getInstance().subscribe(new Listener<>(event -> recreate()));
    }

    public void recreate() {
        if (mc.getWindow() == null) return;

        fbos.forEach(RenderTarget::destroyBuffers);
        fbos.clear();
        menuReady = false;
        lastBlur = 0L;
        if (menuSource != null) {
            menuSource.destroyBuffers();
            menuSource = null;
        }

        // Dual-filter Kawase: each level is half the size of the one before it. Every pass used to
        // run at full window resolution, which cost several times more for the same result.
        for (int i = 0; i <= InterfaceConfig.getPasses(); i++) {
            fbos.add(createFbo(i + 1));
        }
    }

    public void render(PoseStack matrixStack) {
        if (InterfaceConfig.getGlassy() == 1f || fbos.isEmpty()) return;

        // One pass per interval, never a catch-up loop. A hitch used to replay the whole
        // pyramid several times in the same frame, which is what dropped the fps.
        long now = System.nanoTime();
        if (now - lastBlur < 33_000_000L) return;
        lastBlur = now;
        menuReady = false;
        blurFrom(mc.gameRenderer.mainRenderTarget());
    }

    /**
     * Blurs a still cover (the menu wallpaper) into the same targets the panels sample.
     * The image does not move, so this runs once per resize instead of every frame.
     */
    public void renderMenu(GpuTextureView wallpaper, float u0, float v0, float uSpan, float vSpan) {
        if (InterfaceConfig.getGlassy() == 1f || fbos.isEmpty() || wallpaper == null) return;

        int w = Math.max(1, mc.getWindow().getWidth());
        int h = Math.max(1, mc.getWindow().getHeight());
        if (menuReady && menuW == w && menuH == h) return;

        if (menuSource == null || menuSource.width != w || menuSource.height != h) {
            if (menuSource != null) menuSource.destroyBuffers();
            menuSource = new TextureTarget("FestVisuals Menu", w, h, false,
                    mc.gameRenderer.mainRenderTarget().getColorTexture().getFormat());
        }

        FestRenderer.getInstance().fullscreen(FestPipelines.MENU_COVER, menuSource, wallpaper, FestTextures.sampler(),
                FestUniform.of(u0, v0, uSpan, vSpan));
        blurFrom(menuSource);
        menuReady = true;
        menuW = w;
        menuH = h;
    }

    private void blurFrom(RenderTarget source) {
        int actualPasses = Math.max(fbos.size() - 1, 1);

        applyBlurPass(FestPipelines.BLUR_DOWNSCALE, source, fbos.getFirst(), 0, actualPasses);

        for (int i = 0; i < actualPasses; i++) {
            applyBlurPass(FestPipelines.BLUR_DOWNSCALE, fbos.get(i), fbos.get(i + 1), i + 1, actualPasses);
        }

        for (int i = actualPasses; i > 0; i--) {
            applyBlurPass(FestPipelines.BLUR_UPSCALE, fbos.get(i), fbos.get(i - 1), i, actualPasses);
        }
    }

    private void applyBlurPass(RenderPipeline pipeline, RenderTarget source, RenderTarget destination, int pass, int actualPasses) {
        // The offset is in source texels. A smaller level has bigger texels, so scale it down to
        // keep the blur the same width on screen as it was at full resolution.
        float texelScale = source.width / (float) Math.max(1, mc.getWindow().getWidth());
        FestRenderer.getInstance().fullscreen(pipeline, destination, source.getColorTextureView(), FestTextures.sampler(), FestUniform.of(
                0f, 0f, 0f, 0f,
                0.5f / source.width, 0.5f / source.height, InterfaceConfig.getOffset() * (pass / (float) actualPasses) * texelScale, 0f
        ));
    }

    private TextureTarget createFbo(int level) {
        int width = Math.max(1, mc.getWindow().getWidth() >> level);
        int height = Math.max(1, mc.getWindow().getHeight() >> level);
        return new TextureTarget("FestVisuals Blur", width, height, false, mc.gameRenderer.mainRenderTarget().getColorTexture().getFormat());
    }
}
