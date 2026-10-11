#include "Hits.h"

#include "Settings.h"

namespace
{
    // HitActor(attacker, target, projectile, leftHand): the melee hit, as the
    // game calls it when a swing connects (SE 1.5.97 628C20, AE 64E760).
    using HitActor_t = void*(RE::Actor*, RE::Actor*, RE::Projectile*, bool);

    // Inside HitActor: the call that fills in the HitData from the weapon.
    using Populate_t = void(RE::HitData*, RE::TESObjectREFR*, RE::TESObjectREFR*, RE::InventoryEntryData*, bool);
    REL::Relocation<Populate_t> _Populate;

    // Set only around our own HitActor call (main thread).
    struct Pending
    {
        bool active = false;
        float scale = 1.0f;
        bool power = false;
    };
    Pending pending;

    void Populate(RE::HitData* a_hit, RE::TESObjectREFR* a_source, RE::TESObjectREFR* a_target, RE::InventoryEntryData* a_weapon, bool a_offhand)
    {
        _Populate(a_hit, a_source, a_target, a_weapon, a_offhand);
        if (!pending.active || !a_hit || a_source != RE::PlayerCharacter::GetSingleton()) {
            return;
        }
        a_hit->totalDamage *= pending.scale;
        a_hit->physicalDamage *= pending.scale;
        if (pending.power) {
            a_hit->flags.set(RE::HitData::Flag::kPowerAttack);
        }
    }
}

void Hits::Install()
{
    REL::Relocation<std::uintptr_t> hitActor{ RELOCATION_ID(37673, 38627) };
    auto& trampoline = SKSE::GetTrampoline();
    _Populate = trampoline.write_call<5>(hitActor.address() + REL::Relocate(0x1B7, 0x1C6), Populate);
    logger::info("Hits: hooked the melee hit's damage (Elden Ring motion values)");
}

void Hits::Strike(RE::PlayerCharacter* player, RE::Actor* target, float motion, bool leftHand, bool power)
{
    static REL::Relocation<HitActor_t> hitActor{ RELOCATION_ID(37673, 38627) };
    pending = { true, std::max(motion, 0.0f) * Settings::Get().damageScale, power };
    hitActor(player, target, nullptr, leftHand);
    pending = {};
}
