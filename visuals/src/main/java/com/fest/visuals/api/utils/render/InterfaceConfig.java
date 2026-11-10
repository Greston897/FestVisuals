package com.fest.visuals.api.utils.render;

import lombok.experimental.UtilityClass;

/**
 * Global look-and-feel constants for the client's own rendering: overlay scale and the
 * parameters of the Kawase blur that backs every frosted panel.
 */
@UtilityClass
public class InterfaceConfig {
    /** Multiplier applied on top of the resolution-derived overlay scale. */
    public float getScale() { return 0.9f; }

    /** Tint mixed over the liquid-glass lens. Lower leaves more of the scene visible. */
    public float getGlassy() { return 0.32f; }

    /** Kawase down/up sample passes. More passes widen the blur at some GPU cost. */
    public int getPasses() { return 3; }

    /** Per-pass sampling offset; drives how far the blur spreads. */
    public float getOffset() { return 12f; }
}
