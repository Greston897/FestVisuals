package com.fest.visuals.api.utils.render.pipeline;

import com.mojang.blaze3d.pipeline.BlendFunction;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.pipeline.BindGroupLayout;
import com.mojang.blaze3d.PrimitiveTopology;
import com.mojang.blaze3d.pipeline.DepthStencilState;
import com.mojang.blaze3d.pipeline.ColorTargetState;
import com.mojang.blaze3d.platform.CompareOp;
import com.mojang.blaze3d.shaders.UniformType;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.VertexFormat;
import lombok.experimental.UtilityClass;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.BindGroupLayouts;
import net.minecraft.resources.Identifier;
import com.fest.visuals.api.system.backend.ClientInfo;

@UtilityClass
public class FestPipelines {
    public final String UBO = "EvaParams";
    private final BindGroupLayout EVA_PARAMS = BindGroupLayout.builder().withUniform(UBO, UniformType.UNIFORM_BUFFER).build();
    private final BindGroupLayout INPUT_SAMPLER = BindGroupLayout.builder().withSampler("InSampler").build();
    private final BindGroupLayout MOTION_INPUT_SAMPLERS = BindGroupLayout.builder()
            .withSampler("MainSampler")
            .withSampler("MainDepthSampler")
            .build();

    public final RenderPipeline RECT = RenderPipelines.register(gui("rect", "rect/rect", DefaultVertexFormat.POSITION_COLOR).build());
    public final RenderPipeline GRADIENT_RECT = RenderPipelines.register(gui("gradient_rect", "rect/gradient_rect", DefaultVertexFormat.POSITION_COLOR).build());
    public final RenderPipeline TEXTURE_RECT = RenderPipelines.register(gui("texture_rect", "rect/texture_rect", DefaultVertexFormat.POSITION_TEX_COLOR).build());
    public final RenderPipeline BLURRED_RECT = RenderPipelines.register(gui("blurred_rect", "rect/blurred_rect", DefaultVertexFormat.POSITION_TEX_COLOR).build());
    public final RenderPipeline TEXT_10 = text("text_10", 10.0f);
    public final RenderPipeline TEXT_12 = text("text_12", 12.0f);
    public final RenderPipeline TEXT_32 = text("text_32", 32.0f);
    public final RenderPipeline TEXT = TEXT_32;

    public final RenderPipeline BLUR_DOWNSCALE = fullscreen("blur_downscale", "post/blur/downscale");
    public final RenderPipeline MENU_COVER = fullscreen("menu_cover", "post/cover");
    public final RenderPipeline BLUR_UPSCALE = fullscreen("blur_upscale", "post/blur/upscale");
    public final RenderPipeline MOTION_BLUR = RenderPipelines.register(RenderPipeline.builder(RenderPipelines.POST_PROCESSING_SNIPPET)
            .withLocation(id("pipeline/motion_blur"))
            .withVertexShader(Identifier.withDefaultNamespace("core/screenquad"))
            .withFragmentShader(shader("post/motionblur/motion_blur"))
            .withBindGroupLayout(MOTION_INPUT_SAMPLERS)
            .withBindGroupLayout(EVA_PARAMS)
            .build());

    public void register() {
    }

    private RenderPipeline.Builder gui(String name, String shader, VertexFormat format) {
        RenderPipeline.Builder builder = RenderPipeline.builder(RenderPipelines.GLOBALS_SNIPPET)
                .withBindGroupLayout(BindGroupLayouts.MATRICES_PROJECTION);
        if (format == DefaultVertexFormat.POSITION_TEX_COLOR) {
            builder.withBindGroupLayout(BindGroupLayouts.SAMPLER0);
        }
        return builder.withBindGroupLayout(EVA_PARAMS)
                .withLocation(id("pipeline/" + name))
                .withVertexShader(shader(shader))
                .withFragmentShader(shader(shader))
                .withColorTargetState(new ColorTargetState(BlendFunction.TRANSLUCENT))
                .withCull(false)
                .withDepthStencilState(new DepthStencilState(CompareOp.ALWAYS_PASS, false))
                .withVertexBinding(0, format)
                .withPrimitiveTopology(PrimitiveTopology.QUADS);
    }

    private RenderPipeline fullscreen(String name, String shader) {
        return RenderPipelines.register(RenderPipeline.builder(RenderPipelines.POST_PROCESSING_SNIPPET)
                .withLocation(id("pipeline/" + name))
                .withVertexShader(Identifier.withDefaultNamespace("core/screenquad"))
                .withFragmentShader(shader(shader))
                .withBindGroupLayout(INPUT_SAMPLER)
                .withBindGroupLayout(EVA_PARAMS)
                .build());
    }

    private RenderPipeline text(String name, float range) {
        return RenderPipelines.register(RenderPipeline.builder(RenderPipelines.GUI_TEXTURED_SNIPPET)
                .withLocation(id("pipeline/" + name))
                .withFragmentShader(shader("msdf_text"))
                .withShaderDefine("MSDF_RANGE", range)
                .withBindGroupLayout(EVA_PARAMS)
                .build());
    }

    private Identifier shader(String path) {
        return Identifier.fromNamespaceAndPath(ClientInfo.NAME.toLowerCase(), "core/" + path);
    }

    private Identifier id(String path) {
        return Identifier.fromNamespaceAndPath(ClientInfo.NAME.toLowerCase(), path);
    }
}
