Rust Primesieves
----------------

This project contains 2 examples of prime number finders / generators. Both are variants of the seive of eratosthenes.
One example finds primes using the cpu, the other uses a gpu.

The sieve is represented using an array of u32s for the gpu primesieve, u64s for the cpu primesieve. Each bit of the u32 indicates whether a number is prime or not.

An example of the cpu sieve after calculating primes:
```
0010100000100000100010100010000010100000100010100010100010101100
                                                    ^-- 12th bit which is a prime
1000000000000010001010001010001000000010000010001000001010001000
                              ^--- 98th bit which is a prime
1000000000101000001000001000100000100000101000000000101000001000
```

The first bit represents the number 0. the 12th bit remembers the number 11.
If we want to check if a number n in the sieve is prime, one way is to perform the check `(sieve[n / 64] >> n % 64 & 1) == 1`.
N.B. For the gpu sieve, there's 32 bits per element and the bits are flipped - 0 if prime and 1 otherwise.
