#include "Weapons.h"

namespace
{
    // Keyword -> Elden Ring class. Checked in order before the vanilla type.
    constexpr std::array<std::pair<std::string_view, std::string_view>, 13> kKeywords{ {
        { "WeapTypeKatana"sv, "Uchigatana"sv },
        { "WeapTypeRapier"sv, "Rapier"sv },
        { "WeapTypeWhip"sv, "Whip"sv },
        { "WeapTypeQtrStaff"sv, "Twinblade"sv },
        { "WeapTypeQuarterstaff"sv, "Twinblade"sv },
        { "WeapTypeHalberd"sv, "Halberd"sv },
        { "WeapTypePike"sv, "Great Spear"sv },
        { "WeapTypeSpear"sv, "Short Spear"sv },
        { "WeapTypeClaw"sv, "Claw"sv },
        { "WeapTypeScythe"sv, "Reaper"sv },
        { "WeapTypeCurvedSword"sv, "Curved Sword"sv },
        { "WeapTypeScimitar"sv, "Curved Sword"sv },
        { "WeapTypeWarhammer"sv, "Great Hammer"sv },
    } };

    std::string_view Classify(RE::TESObjectWEAP* weapon)
    {
        for (const auto& [keyword, name] : kKeywords) {
            if (weapon->HasKeywordString(keyword)) {
                return name;
            }
        }
        using T = RE::WEAPON_TYPE;
        switch (weapon->GetWeaponType()) {
        case T::kHandToHandMelee:
            return "Fist"sv;
        case T::kOneHandDagger:
            return "Dagger"sv;
        case T::kOneHandSword:
            return "Longsword"sv;
        case T::kOneHandAxe:
            return "Battle Axe"sv;
        case T::kOneHandMace:
            return "Club"sv;
        case T::kTwoHandSword:
            return "Claymore"sv;
        case T::kTwoHandAxe:
            return "Greataxe"sv;
        default:
            return ""sv;
        }
    }

    bool Ranged(RE::TESObjectWEAP* weapon)
    {
        return weapon->IsBow() || weapon->IsCrossbow() || weapon->IsStaff();
    }

    // One hand: the class name, or "" for an empty hand. Sets supported=false
    // for what stays Skyrim's.
    std::string_view Hand(RE::TESForm* form, bool left, bool& supported)
    {
        if (!form) {
            return "Fist"sv;
        }
        if (form->IsWeapon()) {
            auto* weapon = form->As<RE::TESObjectWEAP>();
            if (Ranged(weapon)) {
                supported = false;
                return ""sv;
            }
            return Classify(weapon);
        }
        if (form->IsArmor()) {
            auto* armor = form->As<RE::TESObjectARMO>();
            return left && armor->IsShield() ? "Shield"sv : "Fist"sv;
        }
        if (form->Is(RE::FormType::Light)) {
            return "Torch"sv;
        }
        if (form->IsMagicItem()) {
            supported = false;
            return ""sv;
        }
        return "Fist"sv;
    }

    struct Applied
    {
        std::int32_t right = -2, left = -2;
        bool twoHanded = false;
        bool operator==(const Applied&) const = default;
    };
    Applied applied;

    std::int32_t Index(std::string_view name)
    {
        if (name.empty()) {
            return -1;
        }
        const std::string s{ name };
        return er_weapon_index(s.c_str());
    }
}

Weapons::Loadout Weapons::Read(RE::PlayerCharacter* player)
{
    Loadout out;
    auto* rightForm = player->GetEquippedObject(false);
    auto* leftForm = player->GetEquippedObject(true);
    out.right = Hand(rightForm, false, out.supported);
    out.left = Hand(leftForm, true, out.supported);
    if (rightForm && rightForm->IsWeapon()) {
        const auto type = rightForm->As<RE::TESObjectWEAP>()->GetWeaponType();
        out.twoHanded = type == RE::WEAPON_TYPE::kTwoHandSword || type == RE::WEAPON_TYPE::kTwoHandAxe;
    }
    // A two-hander fills both hands: Skyrim reports it in each.
    if (out.twoHanded) {
        out.left = ""sv;
    }
    return out;
}

void Weapons::Apply(ErCombat* combat, const Loadout& loadout)
{
    Applied want{ Index(loadout.right), Index(loadout.left), loadout.twoHanded };
    if (want == applied) {
        return;
    }
    applied = want;
    const bool changed = er_set_armaments(combat, want.right, want.left, want.twoHanded) != 0;
    // A class the action table doesn't know (index -1) keeps the current one.
    logger::info("Armaments: right \"{}\" ({}), left \"{}\" ({}), {} -> {}", loadout.right, want.right, loadout.left,
        want.left, loadout.twoHanded ? "two-handed" : "one-handed", changed ? "moveset changed" : "unchanged");
}

void Weapons::Reset()
{
    applied = Applied{};
}
