#include <gtest/gtest.h>
#include "Core/Timestep.hpp"

TEST(Timestep, SecondsToMilliseconds) {
    Voltra::Timestep time(0.5f);

    EXPECT_FLOAT_EQ(time.GetSeconds(), 0.5f);
    EXPECT_FLOAT_EQ(time.GetMilliseconds(), 500.0f);
}

TEST(Timestep, ImplicitConversion) {
    Voltra::Timestep time(1.0f);
    float seconds = time;
    EXPECT_FLOAT_EQ(seconds, 1.0f);
}