#pragma once

// Lands the core's hits through Skyrim's own melee hit: the engine function
// the game calls when a weapon swing connects (HitActor). It builds the hit
// from the weapon in hand with the player's skill, perks, smithing and
// enchantments, then the target takes it the usual way: blocking, armour,
// stagger, blood, impact sounds, aggro, crime and death.
//
// The Elden Ring motion value of the swing scales that damage: a hook right
// after the engine fills in the hit multiplies it, only for our own calls.
namespace Hits
{
    // Needs the SKSE trampoline (allocated in main).
    void Install();

    // motion: the swing's Elden Ring motion value (1.0 = plain weapon damage).
    // power: count it as a power attack (heavy, charged, jump or counter).
    void Strike(RE::PlayerCharacter* player, RE::Actor* target, float motion, bool leftHand, bool power);
}
