/* C interface to er_combat_core.dll. Must match core/src/ffi.rs exactly.
 *
 * Everything is in Skyrim's space: game units (70 to the metre), Z up,
 * heading in radians with 0 facing +Y and turning clockwise toward +X.
 */
#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define ER_SWEEP_STEPS 5

typedef struct ErVec3 {
    float x, y, z;
} ErVec3;

typedef struct ErButton {
    uint8_t held;
    uint8_t pressed;  /* went down since the last er_update */
    uint8_t released; /* went up since the last er_update */
} ErButton;

typedef struct ErInput {
    float move_x, move_y; /* stick, -1..1, x right, y forward, camera-relative */
    float camera_heading;
    ErButton dodge, jump, light, heavy, guard;
    uint8_t crouch, walk;
    uint8_t lock, two_hand_right, two_hand_left; /* one-shot: pressed this frame */
} ErInput;

typedef struct ErWorld {
    ErVec3 position; /* where the player actually is */
    float ground_z;
    uint8_t has_target;
    ErVec3 target;
    uint8_t in_combat;
    float heading;          /* the player's facing in Skyrim */
    uint8_t follow_heading; /* Skyrim turns the player this frame: the core takes `heading`.
                               Otherwise the core owns the facing; apply ErOutput.heading. */
} ErWorld;

typedef enum ErState : uint32_t {
    ER_STATE_GROUND = 0,
    ER_STATE_ACTION = 1,
    ER_STATE_AIR = 2,
    ER_STATE_DEAD = 3,
} ErState;

typedef enum ErAction : uint32_t {
    ER_ACTION_NONE = 0,
    ER_ACTION_ROLL = 1,
    ER_ACTION_CROUCH_ROLL = 2,
    ER_ACTION_BACKSTEP = 3,
    ER_ACTION_SPRINT_STOP = 4,
    ER_ACTION_JUMP = 5,
    ER_ACTION_LAND = 6,
    ER_ACTION_LIGHT_ATTACK = 7,
    ER_ACTION_HEAVY_CHARGE = 8,
    ER_ACTION_HEAVY_ATTACK = 9,
    ER_ACTION_RUN_ATTACK = 10,
    ER_ACTION_ROLL_ATTACK = 11,
    ER_ACTION_BACKSTEP_ATTACK = 12,
    ER_ACTION_CROUCH_ATTACK = 13,
    ER_ACTION_JUMP_ATTACK_LAND = 14,
    ER_ACTION_GUARD_COUNTER = 15,
    ER_ACTION_LEFT_ATTACK = 16,
    ER_ACTION_GUARD_HIT = 17,
    ER_ACTION_GUARD_BREAK = 18,
    ER_ACTION_HURT = 19,
    ER_ACTION_COUNT = 20,
} ErAction;

typedef struct ErOutput {
    ErVec3 position;
    float heading;
    ErState state;
    char action[32]; /* action-table clip name, "" when none */
    float action_frame;
    ErAction action_kind;
    uint32_t action_dir;    /* 0 front, 1 back, 2 left, 3 right */
    uint32_t action_serial; /* +1 each time a new action starts */
    float speed;
    uint8_t sprinting, crouching;
    uint8_t guarding; /* raising or holding the guard */
    uint8_t invincible, airborne;
    uint8_t interruptible; /* Activate may go through to Skyrim */
    float stamina;         /* 0..1 */
    uint8_t hit_live;
    ErVec3 sweep_a[ER_SWEEP_STEPS];
    ErVec3 sweep_b[ER_SWEEP_STEPS];
    float hit_radius;
    float hit_damage;
    float hit_stop;
} ErOutput;

typedef enum ErHurt : uint32_t {
    ER_HURT_SMALL = 0,
    ER_HURT_MIDDLE = 1,
    ER_HURT_LARGE = 2,
    ER_HURT_KNOCKDOWN = 3,
} ErHurt;

typedef enum ErHitResult : uint32_t {
    ER_HIT_IGNORED = 0,
    ER_HIT_DODGED = 1,
    ER_HIT_JUMPED = 2,
    ER_HIT_BLOCKED = 3,
    ER_HIT_GUARD_BROKEN = 4,
    ER_HIT_HIT = 5,
} ErHitResult;

/* er_load_data results. */
typedef enum ErLoadResult : int32_t {
    ER_LOAD_OK = 0,
    ER_LOAD_BAD_PATH = 1,       /* null or not UTF-8 */
    ER_LOAD_IO = 2,             /* can't open or read the file (missing?) */
    ER_LOAD_NOT_ERDATA = 3,     /* not an .erdata file */
    ER_LOAD_VERSION = 4,        /* another version of the format */
    ER_LOAD_LAYOUT = 5,         /* written by a core with different action lists */
    ER_LOAD_CORRUPT = 6,        /* cut short or damaged */
    ER_LOAD_ALREADY_LOADED = 7, /* a table is already loaded; it stays */
} ErLoadResult;

typedef struct ErCombat ErCombat;

/* Loads the Elden Ring action table from an .erdata file (UTF-8 path),
 * replacing the one compiled in. Call once, before er_create. On failure
 * nothing changes and the compiled-in (stand-in) table stays. Returns an
 * ErLoadResult. */
int32_t er_load_data(const char* path);
ErCombat* er_create(ErVec3 position);
void er_destroy(ErCombat* combat);
void er_update(ErCombat* combat, float dt, const ErInput* input, const ErWorld* world, ErOutput* out);
ErHitResult er_receive_hit(ErCombat* combat, ErVec3 from, uint8_t low, ErHurt hurt);
void er_confirm_hit(ErCombat* combat);

#ifdef __cplusplus
}
#endif
