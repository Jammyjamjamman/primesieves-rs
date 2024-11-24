GPU Primesieve
--------------

This sieve starts off by generating "starter primes" up to 65535, or 2^16-1 on the cpu using a simple prime number generator. The gpu then uses these primes to mark off whether every other number up to 2^32-1 is a prime or not.
These primes are passed to the gpu / shader code through the "starter_primes" buffer. This is a uniform buffer because it only needs to be readonly and it has to be of type array<vec4<u32>> because of size restrictions of the uniform buffer. The uniform buffer is typically faster to access than the storage buffer.
Because of limited availability of memory on the gpu, the primesieves are calculated in 64 chunks of the sieve.
The shader code splits the sieve into 2 sections. First, it 'sieves' the bits which represent multiples of the first 11 primes. These primes are less than 32, so more than 1 bit can be marked in a single u32 value. If the prime is 2, 16 bits need marking in a single u32, hence the loop marking bits iterating up to 16. The second part of the shader checks for bits which represents multiples of the last 6531 supplied primes, that are greater than 32.
