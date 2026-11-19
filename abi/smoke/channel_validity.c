#include <stdint.h>

int32_t uvk5_channel_validity(
    uint16_t channel,
    uint8_t band,
    uint8_t scan_list_1,
    uint8_t scan_list_2,
    uint8_t check_scan_list,
    uint8_t scan_list,
    uint8_t priority_channel_1,
    uint8_t priority_channel_2,
    uint8_t *out_valid);

struct test_case {
    uint16_t channel;
    uint8_t band;
    uint8_t scan_list_1;
    uint8_t scan_list_2;
    uint8_t check_scan_list;
    uint8_t scan_list;
    uint8_t priority_channel_1;
    uint8_t priority_channel_2;
    uint8_t expected;
};

int main(void)
{
    static const struct test_case cases[] = {
        {0, 0, 0, 0, 0, 0, 0, 0, 1},
        {199, 6, 0, 0, 0, 0, 0, 0, 1},
        {200, 0, 0, 0, 0, 0, 0, 0, 0},
        {UINT16_MAX, 0, 0, 0, 0, 0, 0, 0, 0},
        {199, 7, 0, 0, 0, 0, 0, 0, 0},
        {2, 0, 1, 0, 1, 0, 10, 11, 1},
        {3, 0, 1, 0, 1, 0, 3, 11, 0},
        {5, 0, 0, 1, 1, 1, 10, 11, 1},
        {6, 0, 0, 1, 1, 1, 10, 6, 0},
        {4, 0, 0, 0, 1, 2, 10, 11, 1},
    };

    for (unsigned int i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        const struct test_case *test = &cases[i];
        uint8_t actual = UINT8_MAX;
        if (uvk5_channel_validity(
                test->channel,
                test->band,
                test->scan_list_1,
                test->scan_list_2,
                test->check_scan_list,
                test->scan_list,
                test->priority_channel_1,
                test->priority_channel_2,
                &actual) != 0 ||
            actual != test->expected) {
            return (int)(i + 1);
        }
    }

    uint8_t unchanged = 42;
    if (uvk5_channel_validity(0, 0, 2, 0, 0, 0, 0, 0, &unchanged) != 1 ||
        unchanged != 42 ||
        uvk5_channel_validity(0, 0, 0, 0, 0, 0, 0, 0, 0) != 1) {
        return 99;
    }
    return 0;
}
