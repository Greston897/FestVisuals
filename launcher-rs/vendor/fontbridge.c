#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_MULTIPLE_MASTERS_H
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    FT_Library library;
    FT_Face face;
    int px;
    int weight;
    FT_Fixed coords[4];
    FT_UInt ncoords;
} FvFont;

void *fv_font_open(const unsigned char *data, long len) {
    FvFont *font = calloc(1, sizeof *font);
    if (!font) return NULL;
    if (FT_Init_FreeType(&font->library)) {
        free(font);
        return NULL;
    }
    if (FT_New_Memory_Face(font->library, data, (FT_Long)len, 0, &font->face)) {
        FT_Done_FreeType(font->library);
        free(font);
        return NULL;
    }
    FT_MM_Var *mm = NULL;
    if (FT_Get_MM_Var(font->face, &mm) == 0 && mm) {
        font->ncoords = mm->num_axis < 4 ? mm->num_axis : 4;
        for (FT_UInt i = 0; i < font->ncoords; i++) font->coords[i] = mm->axis[i].def;
        FT_Done_MM_Var(font->library, mm);
    }
    return font;
}

void fv_font_close(void *raw) {
    FvFont *font = raw;
    if (!font) return;
    if (font->face) FT_Done_Face(font->face);
    if (font->library) FT_Done_FreeType(font->library);
    free(font);
}

static void style(FvFont *font, int px, int weight) {
    if (font->px == px && font->weight == weight) return;
    FT_Set_Pixel_Sizes(font->face, 0, (FT_UInt)(px < 1 ? 1 : px));
    FT_MM_Var *mm = NULL;
    if (font->ncoords && FT_Get_MM_Var(font->face, &mm) == 0 && mm) {
        for (FT_UInt i = 0; i < font->ncoords && i < mm->num_axis; i++) {
            FT_Fixed value = mm->axis[i].def;
            if (mm->axis[i].tag == FT_MAKE_TAG('w', 'g', 'h', 't')) {
                value = ((FT_Fixed)weight) << 16;
                if (value < mm->axis[i].minimum) value = mm->axis[i].minimum;
                if (value > mm->axis[i].maximum) value = mm->axis[i].maximum;
            }
            font->coords[i] = value;
        }
        FT_Set_Var_Design_Coordinates(font->face, font->ncoords, font->coords);
        FT_Done_MM_Var(font->library, mm);
    }
    font->px = px;
    font->weight = weight;
}

int fv_glyph(void *raw, uint32_t code, int px, int weight,
             unsigned char **out, int *w, int *h, int *left, int *top, int *advance) {
    FvFont *font = raw;
    style(font, px, weight);
    if (FT_Load_Char(font->face, code, FT_LOAD_RENDER)) return -1;
    FT_GlyphSlot slot = font->face->glyph;
    int width = (int)slot->bitmap.width;
    int rows = (int)slot->bitmap.rows;
    unsigned char *buf = NULL;
    if (width > 0 && rows > 0 && slot->bitmap.buffer) {
        buf = malloc((size_t)width * (size_t)rows);
        if (!buf) return -1;
        int pitch = slot->bitmap.pitch < 0 ? -slot->bitmap.pitch : slot->bitmap.pitch;
        for (int y = 0; y < rows; y++) {
            memcpy(buf + (size_t)y * (size_t)width, slot->bitmap.buffer + (size_t)y * (size_t)pitch, (size_t)width);
        }
    }
    *out = buf;
    *w = width;
    *h = rows;
    *left = slot->bitmap_left;
    *top = slot->bitmap_top;
    *advance = (int)(slot->advance.x >> 6);
    return 0;
}

void fv_free(void *p) { free(p); }
