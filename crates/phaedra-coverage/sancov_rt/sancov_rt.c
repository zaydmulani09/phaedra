/*
 * phaedra sancov runtime shim
 * Link this into your target binary to enable coverage-guided fuzzing with Phaedra.
 * Compile: cc -O2 -shared -fPIC -o libsancov_rt.so sancov_rt.c
 * Or static: cc -O2 -c -o sancov_rt.o sancov_rt.c && ar rcs libsancov_rt.a sancov_rt.o
 */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <sys/shm.h>
#include <sys/types.h>
#endif

#define PHAEDRA_MAP_SIZE (1 << 16)  /* 65536 edges */
#define PHAEDRA_SHM_ENV  "__PHAEDRA_SHM_ID"

static uint8_t  *phaedra_map     = NULL;
static uint32_t  phaedra_map_size = PHAEDRA_MAP_SIZE;
static uint32_t  guard_count      = 0;

/* Called once at startup by the instrumented binary */
void __sanitizer_cov_trace_pc_guard_init(uint32_t *start, uint32_t *stop) {
    if (start == stop || *start) return;

    const char *shm_id_str = getenv(PHAEDRA_SHM_ENV);
    if (shm_id_str) {
#ifdef _WIN32
        HANDLE hMapFile = OpenFileMappingA(FILE_MAP_ALL_ACCESS, FALSE, shm_id_str);
        if (hMapFile != NULL) {
            phaedra_map = (uint8_t *)MapViewOfFile(hMapFile, FILE_MAP_ALL_ACCESS, 0, 0, PHAEDRA_MAP_SIZE);
        }
#else
        int shm_id = atoi(shm_id_str);
        phaedra_map = (uint8_t *)shmat(shm_id, NULL, 0);
        if (phaedra_map == (uint8_t *)-1) phaedra_map = NULL;
#endif
    }

    if (!phaedra_map) {
        /* Fallback: allocate locally (no coverage feedback to fuzzer) */
        phaedra_map = (uint8_t *)calloc(PHAEDRA_MAP_SIZE, 1);
    }

    /* Assign sequential IDs to guards */
    uint32_t id = 0;
    for (uint32_t *x = start; x < stop; x++) {
        *x = (id++ % PHAEDRA_MAP_SIZE) + 1;
    }
    guard_count = id;
}

/* Called on every edge hit */
void __sanitizer_cov_trace_pc_guard(uint32_t *guard) {
    if (!*guard || !phaedra_map) return;
    uint32_t idx = *guard - 1;
    phaedra_map[idx % PHAEDRA_MAP_SIZE]++;
    if (phaedra_map[idx % PHAEDRA_MAP_SIZE] == 0)
        phaedra_map[idx % PHAEDRA_MAP_SIZE] = 1; /* prevent wrap to zero */
}
