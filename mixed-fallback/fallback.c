#include <stdint.h>

uint8_t mixed_c_channel_valid(uint16_t channel)
{
    return (uint8_t)(channel <= UINT16_C(199));
}
