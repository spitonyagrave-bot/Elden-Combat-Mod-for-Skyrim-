#include "Bridge.h"

#include "Hits.h"
#include "Input.h"
#include "Integrations.h"
#include "Movement.h"
#include "PoseDriver.h"
#include "Settings.h"
#include "Weapons.h"

static_assert(sizeof(ErInput) == 32);
static_assert(sizeof(ErWorld) == 44);
static_assert(offsetof(ErWorld, follow_heading) == 40);
static_assert(sizeof(ErOutput) == 224);
static_assert(offsetof(ErOutput, hit_motion) == 220);
static_assert(offsetof(ErOutput, sweep_a) == 88);
static_assert(offsetof(ErOutput, air_attack) == 78);

namespace
{
    float WrapAngle(float a)
    {
        constexpr float kPi = 3.14159265f;
        while (a > kPi) {
            a -= 2.0f * kPi;
        }
        while (a < -kPi) {
            a += 2.0f * kPi;
        }
        return a;
    }

    ErVec3 ToEr(const RE::NiPoint3& p) { return { p.x, p.y, p.z }; }

    constexpr std::array kDirSuffix{ "Forward"sv, "Back"sv, "Left"sv, "Right"sv };

    bool Directional(ErAction kind)
    {
        return kind == ER_ACTION_ROLL || kind == ER_ACTION_CROUCH_ROLL || kind == ER_ACTION_HURT;
    }

    float Dot(const ErVec3& a, const ErVec3& b) { return a.x * b.x + a.y * b.y + a.z * b.z; }
    ErVec3 Sub(const ErVec3& a, const ErVec3& b) { return { a.x - b.x, a.y - b.y, a.z - b.z }; }
    ErVec3 Add(const ErVec3& a, const ErVec3& b, float s) { return { a.x + b.x * s, a.y + b.y * s, a.z + b.z * s }; }

    // Shortest distance between the segments a-b and c-d (the core's own
    // segment_distance, in Skyrim units).
    float SegmentDistance(const ErVec3& a, const ErVec3& b, const ErVec3& c, const ErVec3& d)
    {
        const ErVec3 u = Sub(b, a), v = Sub(d, c), w = Sub(a, c);
        const float uu = Dot(u, u), uv = Dot(u, v), vv = Dot(v, v), uw = Dot(u, w), vw = Dot(v, w);
        const float denom = uu * vv - uv * uv;
        float s = denom > 1e-8f ? std::clamp((uv * vw - vv * uw) / denom, 0.0f, 1.0f) : 0.0f;
        const float t = vv > 1e-8f ? std::clamp((uv * s + vw) / vv, 0.0f, 1.0f) : 0.0f;
        if (uu > 1e-8f) {
            s = std::clamp((uv * t - uw) / uu, 0.0f, 1.0f);
        }
        const ErVec3 p = Add(a, u, s), q = Add(c, v, t), r = Sub(p, q);
        return std::sqrt(Dot(r, r));
    }

    // Moves whose visuals ERCombat.ini gives to Skyrim's own animations
    // ([Pose] Rolls=TKDodge, Jumps=Skyrim). The core still runs them.
    bool SkyrimShows(const ErOutput& out)
    {
        const auto& settings = Settings::Get();
        const auto kind = out.action_kind;
        const bool acting = out.state == ER_STATE_ACTION;
        if (settings.rollsSkyrim && acting &&
            (kind == ER_ACTION_ROLL || kind == ER_ACTION_CROUCH_ROLL || kind == ER_ACTION_BACKSTEP)) {
            return true;
        }
        // A jump attack shows the Elden Ring swing on the airborne body.
        if (settings.jumpsSkyrim && !out.air_attack && (out.state == ER_STATE_AIR ||
                                        (acting && (kind == ER_ACTION_JUMP || kind == ER_ACTION_LAND)))) {
            return true;
        }
        return false;
    }

    bool MenuUp()
    {
        auto* ui = RE::UI::GetSingleton();
        return !ui || ui->GameIsPaused() || ui->IsApplicationMenuOpen() || ui->IsModalMenuOpen() ||
               ui->IsMenuOpen(RE::DialogueMenu::MENU_NAME);
    }
}

void Bridge::Install()
{
    REL::Relocation<std::uintptr_t> vtbl{ RE::VTABLE_PlayerCharacter[0] };
    _Update = vtbl.write_vfunc(0xAD, Update);
    logger::info("Hooked PlayerCharacter::Update");
}

void Bridge::Update(RE::PlayerCharacter* a_this, float a_delta)
{
    _Update(a_this, a_delta);
    Tick(a_this, a_delta);
    PoseDriver::AfterPlayerUpdate(a_this, a_delta);
}

void Bridge::Tick(RE::PlayerCharacter* player, float delta)
{
    auto* input = Input::Get();
    if (input->toggled) {
        input->toggled = false;
        logger::info("Elden Ring mode {}", input->enabled ? "on" : "off");
        RE::DebugNotification(input->enabled ? "Elden Ring Modus: AN" : "Elden Ring Modus: AUS");
        // One key press is enough: switching on also draws the weapon.
        if (auto* actorState = player->AsActorState(); input->enabled && actorState && !actorState->IsWeaponDrawn()) {
            player->DrawWeaponMagicHands(true);
        }
        if (input->enabled && RE::PlayerCamera::GetSingleton()->IsInFirstPerson()) {
            RE::DebugNotification("Elden Ring Modus braucht die Third-Person-Ansicht");
        }
    }

    auto* state = player->AsActorState();
    // Bows, crossbows, magic staffs and spells stay Skyrim's.
    const Weapons::Loadout loadout = Weapons::Read(player);
    if (loadout.supported != lastSupported) {
        lastSupported = loadout.supported;
        logger::info("Equipment {} Elden Ring combat", loadout.supported ? "supports" : "does not support");
    }
    const bool want = input->enabled && loadout.supported && state && state->IsWeaponDrawn() && !MenuUp() &&
                      !player->IsDead() && !player->IsOnMount() && !RE::PlayerCamera::GetSingleton()->IsInFirstPerson();
    if (want != combatMode) {
        SetCombatMode(want);
    }

    if (!combatMode) {
        input->Take(0.0f);  // drop edges so nothing fires on entry
        input->activatePressed = false;
        return;
    }

    std::unique_lock lock(CoreLock());
    if (!combat) {
        combat = er_create(ToEr(player->GetPosition()));
        Weapons::Reset();
    }
    Weapons::Apply(combat, loadout);

    const float cameraHeading = Movement::CameraHeading(player);
    const bool ownMovement = Movement::Active();
    const ErInput controls = input->Take(cameraHeading);
    RE::Actor* target = UpdateLock(player, controls.lock != 0, cameraHeading);
    if (const int flick = Movement::TakeFlick(); flick != 0 && target) {
        target = SwitchTarget(player, target, flick);
    }
    Movement::SetLocked(target != nullptr);
    ErWorld world{};
    world.position = ToEr(player->GetPosition());
    // Standing is the common case, and Skyrim's controller keeps the player
    // on the floor. During a jump, though, Skyrim's own jump carries the
    // player: the core stays airborne for as long as Skyrim has the player
    // in the air (room for the jump attack), and lands when Skyrim does.
    // (Only then: running down stairs must not count as a fall.)
    // Running off a ledge falls too: once Skyrim has had the player in the
    // air for a moment (longer than a step down stairs), the core falls with
    // it instead of running on in the air.
    midairTime = player->IsInMidair() ? midairTime + delta : 0.0f;
    world.ground_z = player->GetPosition().z;
    if (player->IsInMidair() && (lastState == ER_STATE_AIR || (lastState == ER_STATE_ACTION && lastKind == ER_ACTION_JUMP) ||
                                    midairTime > 0.2f)) {
        world.ground_z -= 10000.0f;
    }
    world.in_combat = player->IsInCombat();
    // Our own lock-on (TDM's is switched off in Elden Ring mode).
    world.has_target = target != nullptr;
    if (target) {
        world.target = ToEr(target->GetPosition());
    }
    // On the ground Skyrim (or TDM) turns the player and the core follows.
    // During a roll, attack or jump, and whenever locked on, the core owns
    // the facing: an unlocked roll turns toward the stick and rolls straight
    // ahead, a locked-on one squares up to the target, as in Elden Ring.
    // With [Movement] Enabled=1 the core always owns the facing: it turns
    // the player toward the stick at Elden Ring's turn rates.
    world.heading = player->GetAngleZ();
    world.follow_heading = !ownMovement && lastState == ER_STATE_GROUND && !target;

    ErOutput out{};
    er_update(combat, delta, &controls, &world, &out);
    PoseDriver::SetSource(SkyrimShows(out) ? nullptr : combat);
    lock.unlock();
    // Skyrim's own jump (Jumps=Skyrim) only from the ground, not mid-action.
    if (Settings::Get().jumpsSkyrim) {
        const bool jumpOk = out.state == ER_STATE_GROUND;
        if (jumpOk != skyrimJump) {
            skyrimJump = jumpOk;
            if (auto* controls = RE::PlayerControls::GetSingleton()) {
                controls->jumpHandler->SetInputEventHandlingEnabled(jumpOk);
            }
        }
    }
    const bool posing = PoseDriver::Active();
    const bool acting = out.state != ER_STATE_GROUND && out.state != ER_STATE_DEAD;
    const bool ownYaw = out.state != ER_STATE_DEAD && (ownMovement || acting || target);
    if (ownYaw) {
        // The core turns in 60 Hz steps; above 60 fps the player turns every
        // frame toward the core's facing instead of jumping each tick. A new
        // action (a roll squaring up) or a big gap snaps at once.
        const float diff = WrapAngle(out.heading - shownYaw);
        if (!yawValid || out.action_serial != lastSerial || std::abs(diff) > 1.2f) {
            shownYaw = out.heading;
        } else {
            const float rate = std::max(25.0f, std::abs(diff) * 20.0f) * delta;
            shownYaw = WrapAngle(shownYaw + std::clamp(diff, -rate, rate));
        }
        yawValid = true;
        Movement::Turn(player, shownYaw);
    } else {
        yawValid = false;
    }
    // TDM keeps its hands off the yaw while the core turns the player, and
    // its directional movement pauses while ours runs (and while locked on:
    // Elden Ring strafes round the target, facing it).
    Integrations::Update(ownYaw, out.heading, ownMovement || target != nullptr, target);
    if (posing) {
        ApplyRootMotion(player, world, out, acting);
    } else if (heldMovement) {
        ReleaseMovement();
    }
    if (ownMovement) {
        Walk(player, world, out);
        if (target) {
            const auto to = target->GetPosition() - player->GetPosition();
            Movement::SteerCamera(player, std::atan2(to.x, to.y), delta);
        }
    }
    if (out.state != lastState) {
        logger::info("Core state {} -> {} (action {} \"{}\", stamina {:.2f})", static_cast<std::uint32_t>(lastState),
            static_cast<std::uint32_t>(out.state), static_cast<std::uint32_t>(out.action_kind),
            static_cast<const char*>(out.action), out.stamina);
        lastState = out.state;
    }
    lastKind = out.action_kind;

    if (controls.dodge.pressed || controls.jump.pressed || controls.light.pressed || controls.heavy.pressed ||
        controls.guard.pressed) {
        logger::info("Input: dodge {} jump {} light {} heavy {} guard {} move ({:.2f}, {:.2f}) -> state {} stamina {:.2f}",
            controls.dodge.pressed, controls.jump.pressed, controls.light.pressed, controls.heavy.pressed,
            controls.guard.pressed, controls.move_x, controls.move_y, static_cast<std::uint32_t>(out.state), out.stamina);
    }

    if (out.action_serial != lastSerial) {
        lastSerial = out.action_serial;
        hitThisSwing.clear();
        PlayAction(player, out);
        if (out.action_kind == ER_ACTION_JUMP_ATTACK_LAND) {
            if (Settings::Get().landShake > 0.0f) {
                RE::ShakeCamera(Settings::Get().landShake, player->GetPosition(), 0.35f);
            }
            GroundImpact(player);
        }
    }
    if (out.hit_live && !hitLive) {
        PlaySwing(player, out);
    }

    if (out.hit_live && (posing || Settings::Get().coreHitDetection)) {
        CoreHits(player, out);
    } else if (!out.hit_live && hitLive) {
        // A swing with more than one hit (a spin, a combo in one clip): each
        // hit may strike everyone it reaches once.
        hitThisSwing.clear();
    }
    hitLive = out.hit_live != 0;

    // Guard: raise and lower Skyrim's block, so its blocking (damage, block
    // perks, stamina) and the list's block mods see a raised guard.
    const bool guarding = out.guarding != 0;
    if (guarding != blocking) {
        blocking = guarding;
        SetBlock(player, guarding);
    }

    // I-frames through the graph variables the game and MaxsuIFrame read.
    const bool invincible = out.invincible != 0;
    if (invincible != iframes) {
        iframes = invincible;
        player->SetGraphVariableBool("bIframeActive"sv, invincible);
        player->SetGraphVariableBool("bInIframe"sv, invincible);
    }

    // Activate always goes through: the core says when it would cut an action
    // short, and Skyrim's own activation simply waits for the animation.
    input->activatePressed = false;
}

void Bridge::SetCombatMode(bool on)
{
    combatMode = on;
    Integrations::SetErMode(on);
    Movement::SetActive(on && Settings::Get().ownMovement);
    Input::Get()->padOwned = on;
    midairTime = 0.0f;
    if (!on) {
        if (auto* player = RE::PlayerCharacter::GetSingleton()) {
            SetSprint(player, false);
        }
    }
    if (!on && heldMovement) {
        ReleaseMovement();
    }
    logger::info("Combat mode {}", on ? "on" : "off");
    if (on) {
        RE::DebugNotification("Elden Ring Kampfsteuerung aktiv");
    }
    if (auto* controls = RE::PlayerControls::GetSingleton()) {
        // The core owns these while a weapon is drawn. Movement, Activate,
        // shouts, sneaking and the camera stay with Skyrim.
        controls->sprintHandler->SetInputEventHandlingEnabled(!on);
        // Jumps=Skyrim: Skyrim's jump handler jumps (from the ground; see Tick).
        controls->jumpHandler->SetInputEventHandlingEnabled(!on || Settings::Get().jumpsSkyrim);
        skyrimJump = !on || Settings::Get().jumpsSkyrim;
        controls->attackBlockHandler->SetInputEventHandlingEnabled(!on);
        // R3 is lock-on in Elden Ring mode, not the first-person toggle (and
        // first person would end Elden Ring combat anyway).
        controls->togglePOVHandler->SetInputEventHandlingEnabled(!on);
    }
    if (!on && lockTarget) {
        lockTarget = RE::ActorHandle();
        logger::info("Lock-on released (combat mode off)");
    }
    if (!on && combat) {
        {
            std::lock_guard lock(CoreLock());
            PoseDriver::SetSource(nullptr);
            er_destroy(combat);
            combat = nullptr;
        }
        lastSerial = 0;
        lastState = ER_STATE_GROUND;
        lastKind = ER_ACTION_NONE;
        if (blocking) {
            blocking = false;
            if (auto* player = RE::PlayerCharacter::GetSingleton()) {
                SetBlock(player, false);
            }
        }
        if (iframes) {
            iframes = false;
            if (auto* player = RE::PlayerCharacter::GetSingleton()) {
                player->SetGraphVariableBool("bIframeActive"sv, false);
                player->SetGraphVariableBool("bInIframe"sv, false);
            }
        }
    }
}

void Bridge::PlayAction(RE::PlayerCharacter* player, const ErOutput& out)
{
    const auto kind = out.action_kind;
    if (kind >= ER_ACTION_COUNT) {
        return;
    }
    if (PoseDriver::Active()) {
        // The Elden Ring animation plays on the skeleton itself. Starting a
        // Skyrim attack or dodge would hand MCO, Precision, TK Dodge and the
        // rest something to act on.
        logger::info("Action {} ({}): Elden Ring pose plays it, no Skyrim event", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action));
        return;
    }
    if (kind == ER_ACTION_JUMP && Settings::Get().jumpsSkyrim) {
        // Skyrim's jump handler already jumped on the same key press.
        logger::info("Action {} ({}): Skyrim's own jump plays it", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action));
        return;
    }
    const auto& base = Settings::Get().events[kind];
    if (base.empty()) {
        logger::info("Action {} ({}): no event set in ERCombat.ini", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action));
        return;
    }
    if (Directional(kind) && out.action_dir < kDirSuffix.size()) {
        const auto named = base + std::string(kDirSuffix[out.action_dir]);
        if (player->NotifyAnimationGraph(named.c_str())) {
            logger::info("Action {} ({}): played {}", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action), named);
            return;
        }
        logger::info("Action {} ({}): no animation answered {}", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action), named);
    }
    if (player->NotifyAnimationGraph(base.c_str())) {
        logger::info("Action {} ({}): played {}", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action), base);
    } else {
        logger::info("Action {} ({}): no animation answered {}", static_cast<std::uint32_t>(kind), static_cast<const char*>(out.action), base);
    }
}

void Bridge::SetBlock(RE::PlayerCharacter* player, bool on)
{
    const auto& event = on ? Settings::Get().blockStart : Settings::Get().blockStop;
    if (event.empty()) {
        return;
    }
    const bool answered = player->NotifyAnimationGraph(event.c_str());
    logger::info("Guard {}: {} {}", on ? "up" : "down", event, answered ? "played" : "had no animation answer");
}

void Bridge::ApplyRootMotion(RE::PlayerCharacter* player, const ErWorld& world, const ErOutput& out, bool acting)
{
    auto* controls = RE::PlayerControls::GetSingleton();
    if (!acting) {
        if (heldMovement) {
            ReleaseMovement();
        }
        return;
    }
    // During a roll, attack or jump the core's root motion moves the player,
    // not Skyrim's walk: hold Skyrim's movement input at rest.
    if (controls) {
        if (!heldMovement) {
            controls->movementHandler->SetInputEventHandlingEnabled(false);
            heldMovement = true;
        }
        controls->data.moveInputVec = RE::NiPoint2(0.0f, 0.0f);
    }
    if (!Settings::Get().rootMotion) {
        return;
    }
    // In the air Skyrim's own jump carries the player: moving the player
    // here would hold the fall (the player hung in the air after a jump
    // attack).
    if (player->IsInMidair()) {
        return;
    }
    const float dx = out.position.x - world.position.x;
    const float dy = out.position.y - world.position.y;
    const float step = std::sqrt(dx * dx + dy * dy);
    // More than a sprinting step in one frame is not root motion (a hitch, a
    // teleport): leave it.
    if (step < 0.01f || step > 60.0f) {
        return;
    }
    RE::NiPoint3 move(dx, dy, 0.0f);
    // Skyrim's collision: walls, rocks, furniture and other actors stop the
    // move, and a glancing wall lets it slide along.
    move = Collide(player, move);
    if (move.x * move.x + move.y * move.y < 0.0001f) {
        return;
    }
    RE::NiPoint3 pos = player->GetPosition();
    pos.x += move.x;
    pos.y += move.y;
    player->SetPosition(pos, true);
}

RE::NiPoint3 Bridge::Collide(RE::PlayerCharacter* player, RE::NiPoint3 move)
{
    auto* cell = player->GetParentCell();
    auto* world = cell ? cell->GetbhkWorld() : nullptr;
    if (!world) {
        return RE::NiPoint3();  // no physics to ask: don't move
    }
    std::uint32_t filter = 0;
    player->GetCollisionFilterInfo(filter);
    // The player's own collision group (so the ray ignores the player),
    // tested like a character controller.
    filter = (filter & 0xFFFF0000) | static_cast<std::uint32_t>(RE::COL_LAYER::kCharController);
    const float scale = RE::bhkWorld::GetWorldScale();
    constexpr float kRadius = 30.0f;  // a little more than the player's capsule
    constexpr std::array kHeights{ 40.0f, 75.0f, 110.0f };  // above a stair step, hips, chest

    // Up to two passes: straight, then sliding along what was hit.
    for (int pass = 0; pass < 2; ++pass) {
        const float length = std::sqrt(move.x * move.x + move.y * move.y);
        if (length < 0.01f) {
            return RE::NiPoint3();
        }
        const RE::NiPoint3 dir(move.x / length, move.y / length, 0.0f);
        float allowed = length;
        RE::NiPoint3 normal;
        bool blocked = false;
        const auto base = player->GetPosition();
        for (const float h : kHeights) {
            const RE::NiPoint3 from(base.x, base.y, base.z + h);
            const RE::NiPoint3 to = from + dir * (length + kRadius);
            RE::bhkPickData pick;
            pick.rayInput.from = RE::hkVector4(from * scale);
            pick.rayInput.to = RE::hkVector4(to * scale);
            pick.rayInput.filterInfo = filter;
            {
                RE::BSReadLockGuard lock(world->worldLock);
                world->PickObject(pick);
            }
            if (!pick.rayOutput.HasHit()) {
                continue;
            }
            const float free = pick.rayOutput.hitFraction * (length + kRadius) - kRadius;
            if (free < allowed) {
                allowed = std::max(free, 0.0f);
                alignas(16) float n[4];
                _mm_store_ps(n, pick.rayOutput.normal.quad);
                normal = RE::NiPoint3(n[0], n[1], 0.0f);
                blocked = true;
            }
        }
        if (!blocked) {
            return move;
        }
        if (pass == 1) {
            return dir * allowed;
        }
        // Go as far as the obstacle allows, then slide the rest along it.
        const RE::NiPoint3 done = dir * allowed;
        RE::NiPoint3 rest = move - done;
        const float nl = std::sqrt(normal.x * normal.x + normal.y * normal.y);
        if (nl < 1e-4f) {
            return done;
        }
        normal = normal / nl;
        const float into = rest.x * normal.x + rest.y * normal.y;
        if (into < 0.0f) {
            rest = rest - normal * into;
        }
        // Check the slid move again (from the start: the part before the
        // obstacle is short and clear).
        move = done + rest;
    }
    return move;
}

void Bridge::Walk(RE::PlayerCharacter* player, const ErWorld& world, const ErOutput& out)
{
    if (heldMovement) {
        // Root motion moves the player during this action.
        SetSprint(player, false);
        Movement::SetMove(0.0f, 0.0f);
        return;
    }
    // Where the core ran this frame, in the player's own frame (Skyrim's
    // heading: 0 along +Y, turning clockwise toward +X).
    const float dx = out.position.x - world.position.x;
    const float dy = out.position.y - world.position.y;
    const float step = std::sqrt(dx * dx + dy * dy);
    if (out.speed < 5.0f || out.state == ER_STATE_ACTION) {
        SetSprint(player, false);
        Movement::SetMove(0.0f, 0.0f);
        walkDirValid = false;
        return;
    }
    // Above 60 fps most frames have no simulation tick and no step: keep
    // running the way the last tick went (stopping on those frames made the
    // run stutter and crawl).
    if (step >= 1e-3f) {
        walkDirX = dx / step;
        walkDirY = dy / step;
        walkDirValid = true;
    } else if (!walkDirValid) {
        Movement::SetMove(0.0f, 0.0f);
        return;
    }
    const float h = player->GetAngleZ();
    const float fx = std::sin(h), fy = std::cos(h);
    const float wx = walkDirX, wy = walkDirY;
    const float forward = wx * fx + wy * fy;
    const float right = wx * fy - wy * fx;
    // Skyrim walks below about half input and runs above; the core's speed
    // over its run speed picks the gait.
    const float run = std::max(er_gait_speed(1), 1.0f);
    const float amount = std::clamp(out.speed / run, 0.2f, 1.0f);
    Movement::SetMove(right * amount, forward * amount);

    // Skyrim's own sprint (its speed, stamina and character controller), not
    // extra steps on top of its run: those dragged the player along at the
    // old height and sank them into slopes.
    SetSprint(player, out.sprinting && out.state == ER_STATE_GROUND);
}

void Bridge::ReleaseMovement()
{
    heldMovement = false;
    if (auto* controls = RE::PlayerControls::GetSingleton()) {
        controls->movementHandler->SetInputEventHandlingEnabled(true);
    }
}

void Bridge::CoreHits(RE::PlayerCharacter* player, const ErOutput& out)
{
    auto* lists = RE::ProcessLists::GetSingleton();
    if (!lists) {
        return;
    }
    const auto me = player->GetPosition();
    const auto kind = out.action_kind;
    const bool leftHand = kind == ER_ACTION_LEFT_ATTACK;
    const bool power = kind == ER_ACTION_HEAVY_ATTACK || kind == ER_ACTION_JUMP_ATTACK_LAND ||
                       kind == ER_ACTION_GUARD_COUNTER || out.hit_motion >= 1.2f;
    for (auto& handle : lists->highActorHandles) {
        auto ptr = handle.get();
        auto* actor = ptr.get();
        if (!actor || actor == player || actor->IsDead() || !actor->Get3D() || hitThisSwing.contains(handle.native_handle())) {
            continue;
        }
        const auto at = actor->GetPosition();
        if (at.GetDistance(me) > 700.0f) {
            continue;
        }
        // The target's body as a capsule from its own bounds (a draugr, a
        // giant and a skeever differ), padded so a blade that grazes the
        // armour counts, as Skyrim's own swing test does.
        const float scale = actor->GetScale();
        const auto lo = actor->GetBoundMin();
        const auto hi = actor->GetBoundMax();
        const float width = std::max(hi.x - lo.x, hi.y - lo.y) * scale;
        const float bodyRadius = std::clamp(width * 0.5f, 18.0f, 120.0f) + Settings::Get().hitPadding;
        float height = actor->GetHeight();
        if (height < 20.0f) {
            height = 128.0f * scale;
        }
        const ErVec3 feet{ at.x, at.y, at.z + 10.0f };
        const ErVec3 head{ at.x, at.y, at.z + height };
        bool touched = false;
        for (int i = 0; i < ER_SWEEP_STEPS && !touched; ++i) {
            touched = SegmentDistance(out.sweep_a[i], out.sweep_b[i], feet, head) <= out.hit_radius + bodyRadius;
        }
        if (!touched) {
            continue;
        }
        hitThisSwing.insert(handle.native_handle());
        {
            std::lock_guard lock(CoreLock());
            er_confirm_hit(combat);
        }
        const float before = actor->AsActorValueOwner()->GetActorValue(RE::ActorValue::kHealth);
        Hits::Strike(player, actor, out.hit_motion, leftHand, power);
        const float after = actor->AsActorValueOwner()->GetActorValue(RE::ActorValue::kHealth);
        // Heavy weapons and heavy swings always rock the target (Skyrim's
        // own hit doesn't stagger outside power attacks): a flinch for a
        // two-hander's light swing, a full stagger for heavy ones.
        if (!actor->IsDead() && after > 0.0f) {
            auto* weapon = player->GetEquippedObject(leftHand);
            auto* weap = weapon ? weapon->As<RE::TESObjectWEAP>() : nullptr;
            const bool heavyWeapon = weap && (weap->IsTwoHandedSword() || weap->IsTwoHandedAxe());
            if (heavyWeapon || power) {
                const float base = power ? Settings::Get().staggerHeavy : Settings::Get().staggerLight;
                Stagger(player, actor, std::clamp(base * std::max(out.hit_motion, 0.5f), 0.0f, 1.0f));
            }
        }
        RE::ShakeCamera(Settings::Get().hitShake * out.hit_motion, me, 0.25f);
        if (!Settings::Get().hitSound.empty()) {
            RE::PlaySound(Settings::Get().hitSound.c_str());
        }
        logger::info("Core hit {} with the {} hand: motion {:.2f}{}, health {:.0f} -> {:.0f}", actor->GetName(),
            leftHand ? "left" : "right", out.hit_motion, power ? " (power)" : "", before, after);
    }
}

void Bridge::Stagger(RE::PlayerCharacter* player, RE::Actor* target, float magnitude)
{
    if (magnitude <= 0.0f) {
        return;
    }
    // Direction of the blow in the target's frame, 0..1 (0 from the front).
    const auto from = player->GetPosition() - target->GetPosition();
    float angle = WrapAngle(std::atan2(from.x, from.y) - target->GetAngleZ());
    if (angle < 0.0f) {
        angle += 2.0f * 3.14159265f;
    }
    target->SetGraphVariableFloat("staggerDirection"sv, angle / (2.0f * 3.14159265f));
    target->SetGraphVariableFloat("staggerMagnitude"sv, magnitude);
    target->NotifyAnimationGraph("staggerStart");
}

namespace
{
    std::string Lower(std::string v)
    {
        std::transform(v.begin(), v.end(), v.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
        return v;
    }

    struct SwingSound
    {
        std::string name;
        RE::BGSSoundDescriptorForm* form;
    };

    // Every sound descriptor the game knows whose editor ID says "swing",
    // listed once in ERCombat.log (guessed names found nothing last time).
    const std::vector<SwingSound>& SwingSounds()
    {
        static std::vector<SwingSound> sounds;
        static bool scanned = false;
        if (scanned) {
            return sounds;
        }
        scanned = true;
        std::size_t total = 0, named = 0;
        if (auto* data = RE::TESDataHandler::GetSingleton()) {
            for (auto* form : data->GetFormArray<RE::BGSSoundDescriptorForm>()) {
                if (!form) {
                    continue;
                }
                ++total;
                const char* id = form->GetFormEditorID();
                if (!id || !*id) {
                    continue;
                }
                ++named;
                const auto lower = Lower(id);
                if (lower.find("swing") != std::string::npos || lower.find("whoosh") != std::string::npos) {
                    sounds.push_back({ id, form });
                }
            }
        }
        logger::info("Sounds: {} descriptors, {} with editor IDs, {} swing sounds", total, named, sounds.size());
        for (const auto& sound : sounds) {
            logger::info("  swing sound: {} ({:08X})", sound.name, sound.form->GetFormID());
        }
        return sounds;
    }

    // The swing sound that best fits the weapon: blade or blunt, one hand or
    // two, by the words in the sounds' editor IDs.
    RE::BGSSoundDescriptorForm* PickSwing(const RE::TESObjectWEAP* weapon, bool heavy)
    {
        const bool blunt = weapon && (weapon->IsOneHandedMace() || weapon->HasKeywordString("WeapTypeWarhammer"));
        const bool axe = weapon && (weapon->IsOneHandedAxe() || weapon->HasKeywordString("WeapTypeBattleaxe"));
        const bool staff = weapon && weapon->IsStaff();
        RE::BGSSoundDescriptorForm* best = nullptr;
        int bestScore = -1000;
        for (const auto& sound : SwingSounds()) {
            const auto n = Lower(sound.name);
            auto has = [&](const char* word) { return n.find(word) != std::string::npos; };
            int score = 0;
            // Sounds for magic, creatures or bows are not a weapon's whoosh.
            if (has("spell") || has("mag") || has("bow") || has("arrow") || has("dragon") || has("creature") ||
                has("troll") || has("bear") || has("giant") || has("nostamina") || has("fail")) {
                score -= 50;
            }
            if (has("wpn")) {
                score += 5;
            }
            if (!weapon) {
                score += has("unarmed") || has("fist") || has("hand2hand") ? 30 : -10;
            } else {
                if (has("unarmed")) {
                    score -= 20;
                }
                if (blunt || staff) {
                    score += has("blunt") || has("mace") || has("hammer") ? 20 : 0;
                } else if (axe) {
                    score += has("axe") ? 20 : has("blade") ? 8 : 0;
                } else {
                    score += has("blade") || has("sword") ? 20 : 0;
                }
                const bool big = has("2hand") || has("large") || has("heavy") || has("big") || has("great");
                const bool small = has("1hand") || has("small") || has("light") || has("dagger");
                if (heavy) {
                    score += big ? 10 : small ? -5 : 0;
                } else {
                    score += small || has("medium") ? 6 : big ? -3 : 0;
                }
            }
            if (score > bestScore) {
                best = sound.form;
                bestScore = score;
            }
        }
        return best;
    }
}

void Bridge::PlaySwing(RE::PlayerCharacter* player, const ErOutput& out)
{
    const auto& settings = Settings::Get();
    if (!settings.swingSound) {
        return;
    }
    const bool left = out.action_kind == ER_ACTION_LEFT_ATTACK;
    auto* form = player->GetEquippedObject(left);
    auto* weapon = form ? form->As<RE::TESObjectWEAP>() : nullptr;
    auto* audio = RE::BSAudioManager::GetSingleton();
    if (!audio) {
        return;
    }
    RE::BSSoundHandle sound;
    const char* from = nullptr;
    // ERCombat.ini's choice first, then the weapon's own swing sound, then
    // the best match among the game's swing sounds.
    if (!settings.swingSoundID.empty()) {
        if (auto* chosen = RE::TESForm::LookupByEditorID<RE::BGSSoundDescriptorForm>(settings.swingSoundID)) {
            audio->BuildSoundDataFromDescriptor(sound, chosen);
        }
        if (!sound.IsValid()) {
            audio->BuildSoundDataFromEditorID(sound, settings.swingSoundID.c_str(), 0x1A);
        }
        from = "ERCombat.ini SwingSoundID";
    }
    if (!sound.IsValid() && weapon && weapon->attackSound) {
        audio->BuildSoundDataFromDescriptor(sound, weapon->attackSound);
        from = "the weapon's attack sound";
    }
    if (!sound.IsValid()) {
        const bool heavy = (weapon && (weapon->IsTwoHandedSword() || weapon->IsTwoHandedAxe())) || out.hit_motion >= 1.2f;
        if (auto* picked = PickSwing(weapon, heavy)) {
            audio->BuildSoundDataFromDescriptor(sound, picked);
            from = picked->GetFormEditorID();
        }
    }
    if (!sound.IsValid()) {
        // Skyrim's unarmed whoosh (its attack animations call it by name).
        audio->BuildSoundDataFromEditorID(sound, "WPNSwingUnarmed", 0x1A);
        from = "WPNSwingUnarmed";
    }
    if (!sound.IsValid()) {
        if (!swingLogged) {
            swingLogged = true;
            logger::info("Swing sound: none found (set [Impact] SwingSoundID to one of the swing sounds listed above)");
        }
        return;
    }
    if (!swingLogged) {
        swingLogged = true;
        logger::info("Swing sound: {}", from ? from : "?");
    }
    if (auto* node = player->Get3D()) {
        sound.SetObjectToFollow(node);
    }
    sound.Play();
}

void Bridge::GroundImpact(RE::PlayerCharacter* player)
{
    if (!Settings::Get().groundImpact) {
        return;
    }
    auto* form = player->GetEquippedObject(false);
    auto* weapon = form ? form->As<RE::TESObjectWEAP>() : nullptr;
    auto* impacts = RE::BGSImpactManager::GetSingleton();
    if (!weapon || !weapon->impactDataSet || !impacts) {
        return;
    }
    // Straight down from the weapon: the impact for whatever the ground is
    // made of (its sound, dust and mark), as when Skyrim's weapons strike.
    RE::NiPoint3 down(0.0f, 0.0f, -1.0f);
    const bool played = impacts->PlayImpactEffect(player, weapon->impactDataSet, "WEAPON", down, 250.0f, false, false);
    static bool logged = false;
    if (!logged) {
        logged = true;
        logger::info("Ground impact on landing: {}", played ? "played" : "nothing under the weapon");
    }
}

void Bridge::SetSprint(RE::PlayerCharacter* player, bool on)
{
    // What Skyrim's sprint key sets (its sprint handler is held back in
    // Elden Ring mode); Skyrim then sprints while moving forward with stamina.
    // Set every frame: Skyrim clears it when the stamina runs out.
    // The multi-runtime CommonLib build hides playerFlags, so it is reached
    // by its offset (SE 0xBD8, AE 0xBE0; isSprinting is byte 5, bit 0).
    if (REL::Module::IsVR()) {
        return;
    }
    const std::size_t flags = REL::Module::get().version() >= SKSE::RUNTIME_SSE_1_6_629 ? 0xBE0 : 0xBD8;
    auto* bits = reinterpret_cast<std::uint8_t*>(player) + flags + 5;
    *bits = on ? static_cast<std::uint8_t>(*bits | 1u) : static_cast<std::uint8_t>(*bits & ~1u);
}

RE::Actor* Bridge::SwitchTarget(RE::PlayerCharacter* player, RE::Actor* current, int side)
{
    auto* lists = RE::ProcessLists::GetSingleton();
    if (!lists) {
        return current;
    }
    const auto me = player->GetPosition();
    const auto at = current->GetPosition();
    const float base = std::atan2(at.x - me.x, at.y - me.y);
    const float range = Settings::Get().lockRange * 70.0f;
    RE::Actor* best = nullptr;
    float bestAngle = 0.0f;
    for (auto& handle : lists->highActorHandles) {
        auto ptr = handle.get();
        auto* actor = ptr.get();
        if (!actor || actor == player || actor == current || actor->IsDead() || !actor->Get3D()) {
            continue;
        }
        const auto p = actor->GetPosition();
        if (p.GetDistance(me) > range) {
            continue;
        }
        // Heading grows clockwise: right of the current target is positive.
        const float angle = WrapAngle(std::atan2(p.x - me.x, p.y - me.y) - base) * static_cast<float>(side);
        if (angle <= 0.02f) {
            continue;
        }
        if (!best || angle < bestAngle) {
            best = actor;
            bestAngle = angle;
        }
    }
    if (!best) {
        return current;
    }
    lockTarget = best->GetHandle();
    logger::info("Lock-on switched {} to {}", side > 0 ? "right" : "left", best->GetName());
    return best;
}

RE::Actor* Bridge::FindLockTarget(RE::PlayerCharacter* player, float heading)
{
    auto* lists = RE::ProcessLists::GetSingleton();
    if (!lists) {
        return nullptr;
    }
    const auto me = player->GetPosition();
    const float range = Settings::Get().lockRange * 70.0f;
    const RE::NiPoint3 forward(std::sin(heading), std::cos(heading), 0.0f);
    RE::Actor* best = nullptr;
    float bestScore = 0.0f;
    for (auto& handle : lists->highActorHandles) {
        auto ptr = handle.get();
        auto* actor = ptr.get();
        if (!actor || actor == player || actor->IsDead() || !actor->Get3D()) {
            continue;
        }
        RE::NiPoint3 to = actor->GetPosition() - me;
        to.z = 0.0f;
        const float distance = to.Length();
        if (distance > range || distance < 1.0f) {
            continue;
        }
        const float facing = (to.x * forward.x + to.y * forward.y) / distance;  // cos of the angle off the view
        // In front of the camera, within about 60 degrees, nearest the middle
        // and then the closest; enemies first.
        if (facing < 0.5f) {
            continue;
        }
        float score = facing * 2.0f + (1.0f - distance / range);
        if (actor->IsHostileToActor(player)) {
            score += 1.0f;
        }
        if (!best || score > bestScore) {
            best = actor;
            bestScore = score;
        }
    }
    return best;
}

RE::Actor* Bridge::UpdateLock(RE::PlayerCharacter* player, bool pressed, float heading)
{
    RE::Actor* target = nullptr;
    if (lockTarget) {
        auto ptr = lockTarget.get();
        target = ptr.get();
        const char* lost = nullptr;
        if (!target || !target->Get3D()) {
            lost = "target gone";
        } else if (target->IsDead()) {
            lost = "target dead";
        } else if (target->GetPosition().GetDistance(player->GetPosition()) > Settings::Get().lockBreakRange * 70.0f) {
            lost = "out of range";
        }
        if (lost) {
            logger::info("Lock-on lost: {}", lost);
            lockTarget = RE::ActorHandle();
            target = nullptr;
        }
    }
    if (pressed) {
        if (target) {
            logger::info("Lock-on released");
            lockTarget = RE::ActorHandle();
            target = nullptr;
        } else if (auto* found = FindLockTarget(player, heading)) {
            lockTarget = found->GetHandle();
            target = found;
            logger::info("Locked on to {}", found->GetName());
        } else {
            logger::info("Lock-on: nothing to lock on to");
        }
    }
    return target;
}
