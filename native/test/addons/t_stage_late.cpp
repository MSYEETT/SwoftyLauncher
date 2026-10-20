#include "wine_test.hpp"

SWOFTY_ON_LOAD
{
    probe()[33] = reinterpret_cast<intptr_t>(Fake_Stage_Site.get());
    probe()[34] = Fake_Stage_Site();
    probe()[35] = reinterpret_cast<intptr_t>(Fake_Stage_Wrap.get());
    probe()[36] = Fake_Stage_Wrap();
}

SWOFTY_ADDON()
