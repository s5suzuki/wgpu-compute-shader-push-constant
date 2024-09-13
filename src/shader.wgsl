@group(0)
@binding(0)
var<storage, read> v_a: array<f32>;

@group(0)
@binding(1)
var<storage, read_write> v_b: array<f32>; 

struct Pc {
    offset: f32,
}

var<push_constant> pc: Pc;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    v_b[global_id.x] = pc.offset + v_a[global_id.x] + v_b[global_id.x];
}