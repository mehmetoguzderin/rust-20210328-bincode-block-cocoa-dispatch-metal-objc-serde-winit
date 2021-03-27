#include <metal_stdlib>

using namespace metal;

kernel void compute(texture2d<half, access::write> output [[texture(0)]],
                    uint2 gid [[thread_position_in_grid]]) {
    if ((gid.x >= output.get_width()) || (gid.y >= output.get_height())) {
        return;
    }
    output.write(half4(float(gid.x) / float(output.get_width()),
                       float(gid.y) / float(output.get_height()), 1.0, 1.0),
                 gid);
}
