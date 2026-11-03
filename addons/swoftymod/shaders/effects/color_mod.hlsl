#include "swofty/params.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 p = pow(max(to_linear(c.rgb), 0), 1.0 / 2.2);
    p += param_brightness();
    p = (p - 0.5) * param_contrast() + 0.5;
    p = pow(max(p, 0), 1.0 / max(param_gamma(), 0.01));
    float3 rgb = pow(max(p, 0), 2.2);
    float y = luminance(rgb);
    rgb = max(y + (rgb - y) * param_chroma(), 0);
    return float4(from_linear(rgb), c.a);
}
