#version 330

#moj_import <festvisuals:common.glsl>

uniform sampler2D InSampler;

in vec2 texCoord;

out vec4 fragColor;

void main() {
    // uRadius: cover origin (x, y) and span (z, w), matching the menu wallpaper crop.
    // screenquad's texCoord.y is 1 at the top of the target, and the glass sampler
    // reads the top of the screen from v = 1, so the crop's top goes there.
    vec2 uv = vec2(
        uRadius.x + texCoord.x * uRadius.z,
        uRadius.y + (1.0 - texCoord.y) * uRadius.w
    );
    fragColor = vec4(texture(InSampler, clamp(uv, 0.001, 0.999)).rgb, 1.0);
}
