// shader.wgsl
@group(0) @binding(0) var<uniform> byte_coverage: u32;
@group(0) @binding(1) var<uniform> starter_primes: array<vec4<u32>, 1636>;
@group(0) @binding(2) var<storage, read_write> sieve: array<u32>;

@group(1) @binding(0) var<uniform> shift: u32;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) GlobalInvocationID : vec3<u32>) {
    // We're only using x coord for indexing.
    let idx_start: u32 = GlobalInvocationID.x*byte_coverage;
    let len: u32 = arrayLength(&sieve);
    let n_primes: u32 = u32(6542);

    for (var i = idx_start; i < idx_start+byte_coverage; i++) {

        let u32_start = i*32+shift;
        let u32_end = u32_start+31;
        // Handle primes < 32 i.e. can appear more than once in a u32.
        var sieved_u32 = u32(0);
        for (var p_idx: u32 = 0; p_idx < 11; p_idx++) {
            let prime = starter_primes[p_idx / 4][p_idx % 4];
            var mul_start = (u32_start / prime) * prime;
            mul_start += select(u32(0), prime, u32_start > mul_start && (u32(0xFFFFFFFF) - mul_start) >= prime);

            let first_shift = mul_start % 32;
            for (var j = u32(0); j < 16; j++) {
                let u32_shift = j*prime+first_shift;
                sieved_u32 |= select(u32(0), u32(1) << u32_shift, u32_shift < 32);
            }
        }

        // Handle primes > 32
        sieved_u32 |= seive_large_prime(starter_primes[2][3], u32_end);
        sieved_u32 |= seive_large_prime(starter_primes[1635][0], u32_end);
        sieved_u32 |= seive_large_prime(starter_primes[1635][1], u32_end);

        for (var p1_idx: u32 = 3; p1_idx < 1635; p1_idx++) {
            for (var p2_idx: u32 = 0; p2_idx < 4; p2_idx++) {
                let prime = starter_primes[p1_idx][p2_idx];

                sieved_u32 |= seive_large_prime(prime, u32_end);
            }
        }
        sieve[i] = sieved_u32;
    }
}

fn seive_large_prime(prime: u32, check: u32) -> u32 {
    let remainder = check % prime;
    return select(u32(0), u32(0x80000000) >> remainder, remainder < 32);
}
