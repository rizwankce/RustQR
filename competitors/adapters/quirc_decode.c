/* Minimal JSON-lines PGM adapter. Build it only against the pinned quirc tree. */
#include <quirc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int token(FILE *f, char *out, size_t n) {
    int c; size_t i = 0;
    do { c = fgetc(f); } while (c == ' ' || c == '\n' || c == '\r' || c == '\t');
    if (c == '#') { do { c = fgetc(f); } while (c != '\n' && c != EOF); return token(f, out, n); }
    while (c != EOF && c != ' ' && c != '\n' && c != '\r' && c != '\t') { if (i + 1 < n) out[i++] = (char)c; c = fgetc(f); }
    out[i] = '\0'; return i != 0;
}
int main(int argc, char **argv) {
    FILE *f; char value[32]; int w, h, max, i, count = 0; unsigned char *pixels, *dst;
    struct quirc *q; if (argc != 2 || !(f = fopen(argv[1], "rb"))) return 2;
    if (!token(f,value,sizeof value) || strcmp(value,"P5") || !token(f,value,sizeof value)) return 2; w = atoi(value);
    if (!token(f,value,sizeof value)) return 2; h = atoi(value);
    if (!token(f,value,sizeof value)) return 2; max = atoi(value);
    if (w <= 0 || h <= 0 || max != 255) return 2;
    pixels = malloc((size_t)w * (size_t)h); if (!pixels) return 2;
    if (fread(pixels, 1, (size_t)w * (size_t)h, f) != (size_t)w * (size_t)h) return 2; fclose(f);
    q = quirc_new(); if (!q || quirc_resize(q, w, h) < 0) return 2; dst = quirc_begin(q, &w, &h); memcpy(dst, pixels, (size_t)w * (size_t)h); quirc_end(q);
    for (i = 0; i < quirc_count(q); i++) { struct quirc_code code; struct quirc_data data; quirc_extract(q, i, &code); if (!quirc_decode(&code, &data)) { printf("%.*s\n", data.payload_len, data.payload); count++; } }
    quirc_destroy(q); free(pixels); return count ? 0 : 1;
}
