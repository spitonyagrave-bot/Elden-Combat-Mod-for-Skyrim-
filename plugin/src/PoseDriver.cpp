#include "PoseDriver.h"

#include "Bridge.h"
#include "Settings.h"
#include "Win32.h"

#include <set>

static_assert(sizeof(ErXform) == 48);
static_assert(sizeof(ErPoseInfo) == 44);

namespace
{
    // `node`'s transform relative to `root`, from the local transforms down
    // the chain. Worlds may be a frame old right after the animation update;
    // locals are what the animation (or we) just wrote. False if `node` is
    // not under `root`.
    bool RootLocal(RE::NiAVObject* node, RE::NiAVObject* root, RE::NiTransform& out)
    {
        if (!node || !root) {
            return false;
        }
        RE::NiTransform t = node->local;
        for (RE::NiAVObject* p = node->parent; p; p = p->parent) {
            if (p == root) {
                out = t;
                return true;
            }
            t = p->local * t;
        }
        return false;
    }

    ErXform ToEr(const RE::NiTransform& t)
    {
        ErXform x{};
        for (int r = 0; r < 3; ++r) {
            for (int c = 0; c < 3; ++c) {
                x.rot[r * 3 + c] = t.rotate.entry[r][c];
            }
        }
        x.pos[0] = t.translate.x;
        x.pos[1] = t.translate.y;
        x.pos[2] = t.translate.z;
        return x;
    }

    RE::NiMatrix3 Rotation(const ErXform& x)
    {
        RE::NiMatrix3 m;
        for (int r = 0; r < 3; ++r) {
            for (int c = 0; c < 3; ++c) {
                m.entry[r][c] = x.rot[r * 3 + c];
            }
        }
        return m;
    }

    const char* LoadText(std::int32_t result)
    {
        switch (result) {
        case ER_LOAD_BAD_PATH:
            return "the path isn't valid UTF-8";
        case ER_LOAD_IO:
            return "missing or unreadable";
        case ER_LOAD_NOT_ERDATA:
            return "not a baked animation file";
        case ER_LOAD_LAYOUT:
            return "a joint the retarget needs is missing";
        case ER_LOAD_CORRUPT:
            return "cut short or damaged";
        case ER_LOAD_ALREADY_LOADED:
            return "already loaded";
        default:
            return "unknown error";
        }
    }
}

void PoseDriver::Install()
{
    if (!Settings::Get().poseDriver) {
        logger::info("Pose driver off in ERCombat.ini ([Pose] Enabled=0): Skyrim's animations play");
        return;
    }
    auto dir = Win32::PluginDirectory();
    if (!dir) {
        logger::warn("Pose driver: couldn't find the plugin folder for ERCombat.eranims; Skyrim's animations play");
        return;
    }
    auto u8path = (*dir / "ERCombat.eranims").u8string();
    std::string path(reinterpret_cast<const char*>(u8path.c_str()), u8path.size());
    if (auto result = er_load_anims(path.c_str()); result != ER_LOAD_OK) {
        logger::warn("Pose driver: couldn't load {} ({}, code {}); Skyrim's animations play", path, LoadText(result), result);
        return;
    }
    loaded = true;
    rig = er_rig_create();
    logger::info("Pose driver: loaded {} Elden Ring animations from {}", er_anims_clip_count(), path);

    if (Settings::Get().poseHook == 1) {
        REL::Relocation<std::uintptr_t> vtbl{ RE::VTABLE_PlayerCharacter[0] };
        _UpdateAnimation = vtbl.write_vfunc(0x7D, UpdateAnimation);
        logger::info("Pose driver: hooked PlayerCharacter::UpdateAnimation");
    } else {
        logger::info("Pose driver: applying after PlayerCharacter::Update ([Pose] Hook=0)");
    }
}

void PoseDriver::SetSource(const ErCombat* combat)
{
    // Called with Bridge::CoreLock() held.
    if ((combat != nullptr) != wanted) {
        logger::info("Pose driver: {}", combat ? "Elden Ring pose wanted" : "handing back to Skyrim");
    }
    source = combat;
    wanted = combat != nullptr;
}

bool PoseDriver::Active()
{
    return loaded && rig && driven > 0 && !bindFailed && wanted;
}

void PoseDriver::UpdateAnimation(RE::PlayerCharacter* a_this, float a_delta)
{
    _UpdateAnimation(a_this, a_delta);
    if (!sawHook) {
        sawHook = true;
        logger::info("Pose driver: the player's animation update runs (hook live)");
    }
    Apply(a_this, a_delta);
}

void PoseDriver::AfterPlayerUpdate(RE::PlayerCharacter* player, float delta)
{
    if (Settings::Get().poseHook != 1) {
        Apply(player, delta);
    }
}

bool PoseDriver::Bind(RE::PlayerCharacter* player, RE::NiAVObject* root)
{
    boundRoot = root;
    bones.fill(nullptr);
    driven = 0;
    std::array<ErXform, ER_BONE_COUNT> reference{};
    std::array<std::uint8_t, ER_BONE_COUNT> found{};
    std::string missing;
    std::uint32_t foundCount = 0;
    for (std::uint32_t i = 0; i < ER_BONE_COUNT; ++i) {
        const char* name = er_bone_name(i);
        auto* node = root->GetObjectByName(RE::BSFixedString(name));
        RE::NiTransform rel;
        if (node && RootLocal(node, root, rel)) {
            bones[i] = node;
            reference[i] = ToEr(rel);
            found[i] = 1;
            ++foundCount;
        } else {
            missing += missing.empty() ? "" : ", ";
            missing += name;
        }
    }
    driven = er_rig_bind(rig, reference.data(), found.data());
    const auto& pelvis = reference[0];
    logger::info("Pose driver: bound to the player's skeleton: {}/{} bones found, {} driven; pelvis at ({:.1}, {:.1}, {:.1}) root-local",
        foundCount, ER_BONE_COUNT, driven, pelvis.pos[0], pelvis.pos[1], pelvis.pos[2]);
    if (!missing.empty()) {
        logger::warn("Pose driver: not found on the skeleton: {}", missing);
    }
    // Sanity: Skyrim's skeleton should stand up +Z and face +Y in root space.
    if (found[0] && found[5]) {
        logger::info("Pose driver: head is {:.1} units above the pelvis, {:.1} ahead (expect roughly 45 and a few)",
            reference[5].pos[2] - pelvis.pos[2], reference[5].pos[1] - pelvis.pos[1]);
    }
    (void)player;
    if (driven == 0) {
        bindFailed = true;
        logger::warn("Pose driver: nothing to drive on this skeleton; Skyrim's animations play");
        return false;
    }
    bindFailed = false;
    return true;
}

void PoseDriver::Apply(RE::PlayerCharacter* player, float delta)
{
    if (!loaded || !rig || !player) {
        return;
    }
    // Nothing to do: not wanted, and nothing still fading.
    if (!wanted && !writing) {
        return;
    }
    auto* root = player->Get3D(false);
    if (!root) {
        return;
    }
    if (root != boundRoot) {
        if (writing) {
            // The 3D was rebuilt under us: start over, with nothing to fade.
            writing = false;
        }
        if (!wanted) {
            return;
        }
        // Read the reference only while Skyrim's own pose is on the bones,
        // and while standing, so it is a neutral one.
        if (player->IsMoving()) {
            if (!waitLogged) {
                waitLogged = true;
                logger::info("Pose driver: waiting for the player to stand still to read the skeleton");
            }
            return;
        }
        waitLogged = false;
        bindFailed = false;
        Bind(player, root);
    }
    if (bindFailed || driven == 0) {
        return;
    }

    std::array<ErXform, ER_BONE_COUNT> current{};
    for (std::uint32_t i = 0; i < ER_BONE_COUNT; ++i) {
        RE::NiTransform rel;
        if (bones[i] && RootLocal(bones[i], root, rel)) {
            current[i] = ToEr(rel);
        }
    }
    std::array<ErXform, ER_BONE_COUNT> out{};
    std::array<std::uint8_t, ER_BONE_COUNT> drive{};
    ErPoseInfo info{};
    bool any = false;
    {
        // The simulation may be stepped or destroyed on another thread.
        std::lock_guard lock(Bridge::CoreLock());
        any = er_rig_update(rig, wanted ? source : nullptr, delta, current.data(), out.data(), drive.data(), &info) != 0;
    }

    if (wanted) {
        std::string clip(info.clip);
        if (clip != lastClip) {
            lastClip = clip;
            if (info.missing_clip) {
                logger::warn("Pose driver: clip {} is not in ERCombat.eranims; holding the last pose", clip);
            } else {
                logger::info("Pose driver: playing {}", clip);
            }
        }
    }
    if (any != writing) {
        logger::info("Pose driver: {}", any ? "writing the Elden Ring pose" : "faded out; Skyrim's pose is back");
        writing = any;
        if (!any) {
            lastClip.clear();
        }
    }
    if (!any) {
        return;
    }

    RE::NiAVObject* top = nullptr;
    for (std::uint32_t i = 0; i < ER_BONE_COUNT; ++i) {
        auto* node = bones[i];
        if (!drive[i] || !node || !node->parent) {
            continue;
        }
        RE::NiTransform parent;
        if (!RootLocal(node->parent, root, parent)) {
            // The parent is the root itself.
            if (node->parent != root) {
                continue;
            }
            parent = RE::NiTransform();
        }
        node->local.rotate = parent.rotate.Transpose() * Rotation(out[i]);
        if (i == 0) {
            const RE::NiPoint3 target(out[i].pos[0], out[i].pos[1], out[i].pos[2]);
            node->local.translate = parent.Invert() * target;
        }
        if (!top) {
            top = node;
        }
    }
    // Bring the world transforms (and everything hanging off the bones:
    // twist bones, fingers, weapons, armour) in line with the new pose.
    if (top) {
        RE::NiUpdateData update{};
        if (auto* parent = top->parent) {
            parent->Update(update);
        } else {
            top->Update(update);
        }
    }
}
