#include <gtest/gtest.h>
#include "Core/UUID.hpp"
#include <unordered_set>

namespace Voltra {

    TEST(UUIDTest, Generation) {
        UUID id;
        EXPECT_NE(id, 0);
    }

    TEST(UUIDTest, Uniqueness) {
        UUID id1;
        UUID id2;
        UUID id3;

        EXPECT_NE(id1, id2);
        EXPECT_NE(id2, id3);
        EXPECT_NE(id1, id3);
    }

    TEST(UUIDTest, BulkUniqueness) {
        std::unordered_set<uint64_t> ids;
        for (int i = 0; i < 1000; i++) {
            UUID id;
            EXPECT_TRUE(ids.find(id) == ids.end());
            ids.insert(id);
        }
    }
}