#include <metal_stdlib>
using namespace metal;
struct FieldPoint { float4 geometry; float4 color; float4 hsl; float4 shape; };
struct Parameters { float4 background; float4 backgroundHsl; float4 options; float4 shape; };
float3 hsl_rgb(float3 hsl) {
    float3 k = fract(hsl.x + float3(0., 2./3., 1./3.));
    float3 hue = clamp(abs(k*6.-3.)-1., 0., 1.);
    float chroma = (1.-abs(2.*hsl.z-1.))*hsl.y;
    return (hue-0.5)*chroma+hsl.z;
}
kernel void color_field(device const FieldPoint *points [[buffer(0)]], constant Parameters &p [[buffer(1)]],
                        texture2d<float, access::write> target [[texture(0)]], uint2 pixel [[thread_position_in_grid]]) {
    if (pixel.x >= target.get_width() || pixel.y >= target.get_height()) return;
    float2 uv = (float2(pixel)+0.5)/float2(target.get_width(),target.get_height());
    bool normalized = p.options.y > 0.5, hsl = p.options.z > 0.5;
    float3 rgb = p.background.rgb, hs = p.backgroundHsl.xyz;
    float alpha = p.background.a, total = p.options.w, associatedWeight = total*alpha;
    float3 sum = (hsl ? hs : rgb)*associatedWeight;
    for (uint i=0; i<uint(p.options.x); ++i) {
        FieldPoint f=points[i];
        float2 d=uv-f.geometry.xy;
        float x=f.geometry.z*d.x+f.geometry.w*d.y, y=-f.geometry.w*d.x+f.geometry.z*d.y;
        float distance=f.shape.y == 1. ? dot(d,d) : x*x/f.shape.y+y*y*f.shape.y;
        float weight=exp(-pow(max(0.,distance*5./(f.shape.x*f.shape.x)),p.shape.x));
        float a=weight*f.color.a;
        if (!normalized) {
            if (hsl) { hs+=(f.hsl.xyz-hs)*a; rgb=hsl_rgb(hs); }
            else { rgb+=(f.color.rgb-rgb)*a; }
            alpha=a+alpha*(1.-a);
        } else {
            total+=weight; associatedWeight+=a;
            sum+=(hsl ? f.hsl.xyz : f.color.rgb)*a;
        }
    }
    if (normalized) {
        if (total <= 1e-12) { rgb=p.background.rgb; alpha=p.background.a; }
        else if (associatedWeight <= 1e-12) { rgb=0.; alpha=0.; }
        else { rgb=sum/associatedWeight; if(hsl) rgb=hsl_rgb(rgb); alpha=associatedWeight/total; }
    }
    rgb=clamp(rgb,0.,1.); alpha=clamp(alpha,0.,1.);
    if(p.shape.y > 0.5) {
        rgb=select(pow((rgb+0.055)/1.055,float3(2.4)),rgb/12.92,rgb<=0.04045);
        rgb*=alpha;
    }
    target.write(float4(rgb,alpha),pixel);
}
