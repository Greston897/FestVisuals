#version 330

#moj_import <festvisuals:common.glsl>

uniform sampler2D Sampler0;

in vec2 FragCoord;
in vec2 TexCoord;

out vec4 fragColor;

vec4 tintOf(vec2 uv) {
    vec4 top = mix(uTopLeftColor, uTopRightColor, uv.x);
    vec4 bottom = mix(uBottomLeftColor, uBottomRightColor, uv.x);
    return mix(top, bottom, uv.y);
}

vec3 sampleScene(vec2 uv) {
    return texture(Sampler0, clamp(uv, vec2(0.001), vec2(0.999))).rgb;
}

void main() {
    float cover = ralpha(uSize, FragCoord, uRadius, uSmoothness);
    if (cover <= 0.0) {
        discard;
    }

    // TexCoord is this quad's slice of the full-frame Kawase target. Dividing
    // gl_FragCoord by textureSize was wrong: that target is half the window.
    float fade = uText.x;
    float time = uText.y;
    float strength = clamp(uText.z, 0.0, 1.0);
    vec2 uv = TexCoord;
    vec2 texel = 1.0 / vec2(textureSize(Sampler0, 0));

    vec2 pos = (vec2(0.5) - FragCoord) * uSize;
    float sd = rdist(pos, uSize * 0.5 - 1.0, uRadius);
    float reach = mix(14.0, 22.0, strength);
    float rim = clamp((sd + reach) / reach, 0.0, 1.0);
    float rim2 = rim * rim;

    // One tap in the middle. The dark veil is the body; blur stays readable.
    vec3 color = sampleScene(uv);
    vec3 wash = tintOf(FragCoord).rgb;
    color = mix(color, wash, uMix * (1.0 - rim2 * 0.8));

    // Slow shine across the pane. Arithmetic only, so it does not cost samples.
    float sweep = fract(time * 0.07) * 1.7 - 0.35;
    float band = abs(FragCoord.x * 0.96 + FragCoord.y * 0.05 - sweep);
    if (band < 0.01) {
        color += vec3(0.16) * (1.0 - band / 0.01);
    }

    // Refraction, chroma and the prism live on the rim. Interior warps skip this.
    if (rim2 > 0.015) {
        vec2 n = (FragCoord - 0.5) * 2.0;
        float pull = mix(5.0, 9.0, strength);
        vec2 refr = uv - n * rim2 * pull * texel;
        vec3 bent = sampleScene(refr);
        float shift = rim2 * mix(3.0, 6.0, strength);
        bent.r = mix(bent.r, sampleScene(refr + vec2(shift, 0.0) * texel).r, rim);
        bent.b = mix(bent.b, sampleScene(refr - vec2(shift, 0.0) * texel).b, rim);
        color = mix(color, bent, rim2);

        vec2 crest = FragCoord - vec2(0.30, 0.14);
        float crestMask = clamp(1.0 - length(crest * vec2(3.2, 6.6)) / 0.24, 0.0, 1.0);
        color += vec3(crestMask * crestMask * 0.18);

        float hue = fract(atan(n.y, n.x) / 6.2831853 + time * 0.03);
        vec3 prism = clamp(abs(fract(hue + vec3(0.0, 0.67, 0.33)) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
        color = mix(color, prism, rim2 * rim2 * 0.62 * strength);
    }

    fragColor = vec4(color, cover * fade);
}
