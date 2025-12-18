#version 330

//_DEFINES_

uniform sampler2D tex;
uniform float alpha;
in vec2 v_coords;
layout(location = 0) out vec4 fragColor;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// x is left edge, y is right edge of the gradient.
uniform vec2 cutoff;

void main() {
    // Sample the texture.
    vec4 color = texture(tex, v_coords);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif

    if (cutoff.x < cutoff.y) {
        float fade = clamp((cutoff.y - v_coords.x) / (cutoff.y - cutoff.x), 0.0, 1.0);
        color = color * fade;
    }

    // Apply final alpha and tint.
    color = color * alpha;

#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    fragColor = color;
}
