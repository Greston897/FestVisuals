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

    // uText: fade, time, strength, mode. Mode 0 samples the full-screen blur.
    // Mode 1 samples a wallpaper whose UVs were written into TexCoord.
    float fade = uText.x;
    float time = uText.y;
    float strength = clamp(uText.z, 0.0, 1.0);
    float mode = uText.w;

    vec2 pos = (vec2(0.5) - FragCoord) * uSize;
    float sd = rdist(pos, uSize * 0.5 - 1.0, uRadius);
    float reach = mix(16.0, 26.0, strength);
    float rim = clamp((sd + reach) / reach, 0.0, 1.0);
    float rim2 = rim * rim;
    vec2 n = (FragCoord - 0.5) * 2.0;

    vec2 uv;
    vec2 texel;
    if (mode < 0.5) {
        vec2 texSize = vec2(textureSize(Sampler0, 0));
        uv = gl_FragCoord.xy / texSize;
        texel = 1.0 / texSize;
    } else {
        uv = TexCoord;
        texel = vec2(0.010, 0.012);
    }

    float pull = mix(8.0, 14.0, strength);
    vec2 refr = uv - n * rim2 * pull * texel;
    vec3 sharp = sampleScene(refr);
    vec3 soft = sampleScene(refr + n * texel * mix(5.0, 9.0, strength));
    vec3 color = mix(sharp, soft, rim2 * mix(0.40, 0.70, strength));

    float shift = rim2 * mix(5.0, 9.0, strength);
    vec3 red = sampleScene(refr + vec2(shift, 0.0) * texel);
    vec3 blue = sampleScene(refr - vec2(shift, 0.0) * texel);
    color.r = mix(color.r, red.r, rim2);
    color.b = mix(color.b, blue.b, rim2);

    color = mix(color, vec3(0.98), rim2 * mix(0.16, 0.42, strength));

    vec2 crest = FragCoord - vec2(0.32, 0.16);
    float crestMask = clamp(1.0 - length(crest * vec2(3.0, 6.4)) / 0.22, 0.0, 1.0);
    color += (1.0 - color) * crestMask * crestMask * mix(0.18, 0.28, 1.0 - strength * 0.4);

    float sweep = fract(time * 0.11) * 1.7 - 0.35;
    float band = abs(FragCoord.x * 0.96 + FragCoord.y * 0.05 - sweep);
    if (band < 0.008) {
        color += (1.0 - color) * (1.0 - band / 0.008) * 0.62;
    }

    float hue = fract(atan(n.y, n.x) / 6.2831853 + time * 0.04);
    vec3 prism = clamp(abs(fract(hue + vec3(0.0, 0.67, 0.33)) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
    color = mix(color, prism, rim2 * rim2 * 0.40 * strength);

    vec4 wash = tintOf(FragCoord);
    color = mix(color, wash.rgb, uMix);

    fragColor = vec4(color, cover * fade);
}
