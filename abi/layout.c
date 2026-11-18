#include <stddef.h>
#include <stdint.h>

enum LayoutWitnessEnum {
    LAYOUT_WITNESS_ENUM_ZERO = 0,
    LAYOUT_WITNESS_ENUM_ONE = 1,
};

struct Uvk5LayoutWitness {
    int32_t status;
    uint8_t mode;
    uint8_t reserved[3];
    uint32_t frequency_hz;
};

_Static_assert(sizeof(enum LayoutWitnessEnum) == 1,
               "compile this witness with -fshort-enums");
_Static_assert(sizeof(struct Uvk5LayoutWitness) == 12,
               "ABI record size changed");
_Static_assert(_Alignof(struct Uvk5LayoutWitness) == 4,
               "ABI record alignment changed");
_Static_assert(offsetof(struct Uvk5LayoutWitness, status) == 0,
               "status offset changed");
_Static_assert(offsetof(struct Uvk5LayoutWitness, mode) == 4,
               "mode offset changed");
_Static_assert(offsetof(struct Uvk5LayoutWitness, reserved) == 5,
               "reserved offset changed");
_Static_assert(offsetof(struct Uvk5LayoutWitness, frequency_hz) == 8,
               "frequency_hz offset changed");
