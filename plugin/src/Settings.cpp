#include "Settings.h"

#include <Windows.h>

namespace
{
    constexpr auto kPath = L"Data/SKSE/Plugins/ERCombat.ini";

    std::uint32_t ReadKey(const wchar_t* key, std::uint32_t fallback)
    {
        return static_cast<std::uint32_t>(GetPrivateProfileIntW(L"Keys", key, static_cast<int>(fallback), kPath));
    }

    std::string ReadString(const wchar_t* section, const wchar_t* key, const char* fallback)
    {
        wchar_t buffer[128]{};
        std::wstring wideFallback(fallback, fallback + std::strlen(fallback));
        GetPrivateProfileStringW(section, key, wideFallback.c_str(), buffer, 128, kPath);
        std::string out;
        for (const wchar_t* c = buffer; *c; ++c) {
            out.push_back(static_cast<char>(*c));
        }
        // Drop a trailing comment and spaces.
        if (auto semi = out.find(';'); semi != std::string::npos) {
            out.erase(semi);
        }
        while (!out.empty() && (out.back() == ' ' || out.back() == '\t')) {
            out.pop_back();
        }
        return out;
    }

    std::string ReadEvent(const wchar_t* key, const char* fallback)
    {
        return ReadString(L"Events", key, fallback);
    }

    float ReadFloat(const wchar_t* section, const wchar_t* key, float fallback)
    {
        const auto text = ReadString(section, key, "");
        if (text.empty()) {
            return fallback;
        }
        try {
            return std::stof(text);
        } catch (...) {
            return fallback;
        }
    }
}

Settings& Settings::Get()
{
    static Settings settings;
    return settings;
}

void Settings::Load()
{
    toggleKey = ReadKey(L"Toggle", toggleKey);
    dodgeKey = ReadKey(L"Dodge", dodgeKey);
    jumpKey = ReadKey(L"Jump", jumpKey);
    lightKey = ReadKey(L"Light", lightKey);
    guardKey = ReadKey(L"Guard", guardKey);
    heavyModifier = ReadKey(L"HeavyModifier", heavyModifier);
    sprintKey = ReadKey(L"Sprint", sprintKey);
    heavyKey = ReadKey(L"Heavy", heavyKey);
    walkKey = ReadKey(L"Walk", walkKey);
    lockKey = ReadKey(L"LockOn", lockKey);
    lockPad = ReadKey(L"LockOnGamepad", lockPad);
    padDodge = ReadKey(L"PadDodge", padDodge);
    padLight = ReadKey(L"PadLight", padLight);
    padHeavy = ReadKey(L"PadHeavy", padHeavy);
    padGuard = ReadKey(L"PadGuard", padGuard);

    // Defaults drive the animations Gate to Sovngarde already has: TK Dodge
    // for rolls, and the vanilla attack events that MCO/ADXP and BFCO answer.
    events[ER_ACTION_ROLL] = ReadEvent(L"Roll", "TKDodge");
    events[ER_ACTION_CROUCH_ROLL] = ReadEvent(L"CrouchRoll", "TKDodge");
    events[ER_ACTION_BACKSTEP] = ReadEvent(L"Backstep", "TKDodgeBack");
    events[ER_ACTION_JUMP] = ReadEvent(L"Jump", "JumpStandingStart");
    events[ER_ACTION_LIGHT_ATTACK] = ReadEvent(L"LightAttack", "attackStart");
    // Skyrim has no held power attack, so the swing plays when the core
    // releases the charge.
    events[ER_ACTION_HEAVY_CHARGE] = ReadEvent(L"HeavyCharge", "");
    events[ER_ACTION_HEAVY_ATTACK] = ReadEvent(L"HeavyAttack", "attackPowerStartInPlace");
    events[ER_ACTION_RUN_ATTACK] = ReadEvent(L"RunAttack", "attackStartSprint");
    events[ER_ACTION_ROLL_ATTACK] = ReadEvent(L"RollAttack", "attackStart");
    events[ER_ACTION_BACKSTEP_ATTACK] = ReadEvent(L"BackstepAttack", "attackStart");
    events[ER_ACTION_CROUCH_ATTACK] = ReadEvent(L"CrouchAttack", "attackStart");
    events[ER_ACTION_JUMP_ATTACK_LAND] = ReadEvent(L"JumpAttackLand", "attackPowerStartInPlace");
    events[ER_ACTION_GUARD_COUNTER] = ReadEvent(L"GuardCounter", "attackPowerStartInPlace");
    events[ER_ACTION_LEFT_ATTACK] = ReadEvent(L"LeftAttack", "attackStartLeftHand");
    events[ER_ACTION_HURT] = ReadEvent(L"Hurt", "");
    blockStart = ReadEvent(L"BlockStart", "blockStart");
    blockStop = ReadEvent(L"BlockStop", "blockStop");

    poseDriver = GetPrivateProfileIntW(L"Pose", L"Enabled", 1, kPath) != 0;
    poseHook = static_cast<std::int32_t>(GetPrivateProfileIntW(L"Pose", L"Hook", 1, kPath));

    coreHitDetection = GetPrivateProfileIntW(L"General", L"CoreHitDetection", 0, kPath) != 0;
    lockRange = ReadFloat(L"LockOn", L"Range", lockRange);
    lockBreakRange = ReadFloat(L"LockOn", L"BreakRange", lockBreakRange);
    rootMotion = GetPrivateProfileIntW(L"Pose", L"RootMotion", 1, kPath) != 0;
    damageScale = ReadFloat(L"Pose", L"DamageScale", damageScale);
    hitShake = ReadFloat(L"Pose", L"HitShake", hitShake);
    hitPadding = ReadFloat(L"Pose", L"HitPadding", hitPadding);
    hitSound = ReadString(L"Pose", L"HitSound", "");
    ownMovement = GetPrivateProfileIntW(L"Movement", L"Enabled", 1, kPath) != 0;
    lockCameraSpeed = ReadFloat(L"Movement", L"LockCameraSpeed", lockCameraSpeed);
    padLookSmoothing = ReadFloat(L"Movement", L"PadLookSmoothing", padLookSmoothing);
    autoTwoHand = GetPrivateProfileIntW(L"Weapons", L"AutoTwoHand", 1, kPath) != 0;
    lockFlickMouse = ReadFloat(L"LockOn", L"FlickMouse", lockFlickMouse);
    swingSound = GetPrivateProfileIntW(L"Impact", L"SwingSound", 1, kPath) != 0;
    landShake = ReadFloat(L"Impact", L"LandShake", landShake);
    {
        auto lower = [](std::string v) {
            std::transform(v.begin(), v.end(), v.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
            return v;
        };
        rollsSkyrim = lower(ReadString(L"Pose", L"Rolls", "ER")) != "er";
        jumpsSkyrim = lower(ReadString(L"Pose", L"Jumps", "Skyrim")) != "er";
    }
}
