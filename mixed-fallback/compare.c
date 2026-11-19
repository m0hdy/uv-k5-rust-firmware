#include <stdint.h>

uint8_t mixed_c_channel_valid(uint16_t channel);
uint8_t mixed_rust_channel_valid(uint16_t channel);

int main(void)
{
    static const struct {
        uint16_t channel;
        uint8_t expected;
    } cases[] = {
        {0, 1},
        {1, 1},
        {42, 1},
        {198, 1},
        {199, 1},
        {200, 0},
        {UINT16_MAX, 0},
    };

    for (unsigned int i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        if (mixed_c_channel_valid(cases[i].channel) != cases[i].expected ||
            mixed_rust_channel_valid(cases[i].channel) != cases[i].expected) {
            return (int)(i + 1);
        }
    }
    return 0;
}
