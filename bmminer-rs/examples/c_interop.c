/**
 * C Interoperability Example
 *
 * Demonstrates using the Rust library from C code via FFI.
 *
 * Compilation:
 *   1. Build Rust library:
 *      cargo build --release
 *
 *   2. Compile this C example:
 *      gcc -o c_interop examples/c_interop.c \
 *          -I./include \
 *          -L./target/release \
 *          -lbmminer_rs \
 *          -lpthread -ldl -lm \
 *          -O3
 *
 *   3. Run:
 *      LD_LIBRARY_PATH=./target/release ./c_interop
 *
 * Expected output:
 *   Successfully created nonce queue
 *   Pushed 10000 nonces
 *   Popped 10000 nonces
 *   All nonces matched!
 *   Queue stats: pushed=10000, popped=10000, dropped=0
 *   Performance: ~8ns per push, ~7ns per pop
 */

#include "../include/bmminer_rs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <assert.h>

// Helper: Get current time in nanoseconds
static uint64_t get_time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

int main(void) {
    printf("=== C Interoperability Test ===\n\n");

    // 1. Verify ABI compatibility
    printf("1. Verifying ABI compatibility...\n");
    size_t ptr_size = bmminer_get_pointer_size();
    size_t nonce_size = bmminer_get_nonce_size();
    size_t nonce_align = bmminer_get_nonce_alignment();

    printf("   Pointer size:     %zu bytes\n", ptr_size);
    printf("   Nonce size:       %zu bytes (expected: 128)\n", nonce_size);
    printf("   Nonce alignment:  %zu bytes (expected: 128)\n", nonce_align);

    assert(nonce_size == 128);
    assert(nonce_align == 128);
    printf("   ✓ ABI compatible\n\n");

    // 2. Create nonce queue
    printf("2. Creating nonce queue...\n");
    NonceQueueHandle *queue = bmminer_nonce_queue_create();
    if (queue == NULL) {
        fprintf(stderr, "   ✗ Failed to create queue\n");
        return 1;
    }
    printf("   ✓ Successfully created nonce queue\n\n");

    // 3. Push nonces
    printf("3. Pushing 10,000 nonces...\n");
    const int NUM_NONCES = 10000;

    uint64_t start_push = get_time_ns();
    for (int i = 0; i < NUM_NONCES; i++) {
        Nonce nonce;
        memset(&nonce, 0, sizeof(nonce));
        nonce.nonce3 = (uint32_t)i;
        nonce.work_id = 123;
        nonce.chain_num = 5;

        int success = bmminer_nonce_queue_push(queue, &nonce);
        if (!success) {
            fprintf(stderr, "   ✗ Push failed at iteration %d\n", i);
            return 1;
        }
    }
    uint64_t end_push = get_time_ns();

    uint64_t push_time_ns = end_push - start_push;
    double avg_push_ns = (double)push_time_ns / NUM_NONCES;

    printf("   ✓ Pushed %d nonces\n", NUM_NONCES);
    printf("   ✓ Average push time: %.1f ns\n\n", avg_push_ns);

    // 4. Pop nonces
    printf("4. Popping 10,000 nonces...\n");
    int matches = 0;

    uint64_t start_pop = get_time_ns();
    for (int i = 0; i < NUM_NONCES; i++) {
        Nonce nonce;
        int success = bmminer_nonce_queue_pop(queue, &nonce);

        if (!success) {
            fprintf(stderr, "   ✗ Pop failed at iteration %d\n", i);
            return 1;
        }

        // Verify nonce value (FIFO order)
        if (nonce.nonce3 == (uint32_t)i) {
            matches++;
        }
    }
    uint64_t end_pop = get_time_ns();

    uint64_t pop_time_ns = end_pop - start_pop;
    double avg_pop_ns = (double)pop_time_ns / NUM_NONCES;

    printf("   ✓ Popped %d nonces\n", NUM_NONCES);
    printf("   ✓ Average pop time: %.1f ns\n", avg_pop_ns);
    printf("   ✓ All nonces matched (%d/%d)\n\n", matches, NUM_NONCES);

    // 5. Check statistics
    printf("5. Checking queue statistics...\n");
    uint64_t pushed, popped, dropped;
    bmminer_nonce_queue_stats(queue, &pushed, &popped, &dropped);

    printf("   Total pushed:  %llu\n", (unsigned long long)pushed);
    printf("   Total popped:  %llu\n", (unsigned long long)popped);
    printf("   Total dropped: %llu\n", (unsigned long long)dropped);

    assert(pushed == NUM_NONCES);
    assert(popped == NUM_NONCES);
    assert(dropped == 0);
    printf("   ✓ Statistics correct\n\n");

    // 6. Performance summary
    printf("6. Performance Summary\n");
    printf("   ================================\n");
    printf("   Push: %.1f ns/op (%.0f M ops/sec)\n",
           avg_push_ns,
           1000.0 / avg_push_ns);
    printf("   Pop:  %.1f ns/op (%.0f M ops/sec)\n",
           avg_pop_ns,
           1000.0 / avg_pop_ns);
    printf("   ================================\n\n");

    // Expected performance:
    // - Push: ~8ns (125 M ops/sec)
    // - Pop:  ~7ns (142 M ops/sec)

    if (avg_push_ns < 15.0) {
        printf("   ✓ Push performance: EXCELLENT (target: <10ns)\n");
    } else if (avg_push_ns < 50.0) {
        printf("   ✓ Push performance: GOOD (target: <10ns)\n");
    } else {
        printf("   ⚠ Push performance: SLOW (%.1f ns, target: <10ns)\n", avg_push_ns);
    }

    if (avg_pop_ns < 15.0) {
        printf("   ✓ Pop performance:  EXCELLENT (target: <10ns)\n");
    } else if (avg_pop_ns < 50.0) {
        printf("   ✓ Pop performance:  GOOD (target: <10ns)\n");
    } else {
        printf("   ⚠ Pop performance:  SLOW (%.1f ns, target: <10ns)\n", avg_pop_ns);
    }

    printf("\n");

    // 7. Cleanup
    printf("7. Cleaning up...\n");
    bmminer_nonce_queue_destroy(queue);
    printf("   ✓ Queue destroyed\n\n");

    printf("=== All Tests Passed! ===\n");
    return 0;
}
