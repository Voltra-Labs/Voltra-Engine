#include <gtest/gtest.h>
#include "Renderer/Texture.hpp"
#include <vector>

TEST(TextureTest, CreateManualTexture) {
    uint32_t width = 128;
    uint32_t height = 128;
    
    Voltra::Texture2D texture(width, height);
    
    EXPECT_EQ(texture.GetWidth(), width);
    EXPECT_EQ(texture.GetHeight(), height);
    EXPECT_TRUE(texture.IsLoaded());
    EXPECT_NE(texture.GetRendererID(), 0);
}

TEST(TextureTest, SetData) {
    uint32_t width = 2;
    uint32_t height = 2;
    Voltra::Texture2D texture(width, height);
    
    uint32_t whiteData = 0xFFFFFFFF;
    std::vector<uint32_t> data(width * height, whiteData);
    
    EXPECT_NO_THROW(texture.SetData(data.data(), data.size() * sizeof(uint32_t)));
}

TEST(TextureTest, Equality) {
    Voltra::Texture2D t1(1, 1);
    Voltra::Texture2D t2(1, 1);
    
    EXPECT_FALSE(t1 == t2);
    EXPECT_FALSE(t1 == t2);
}