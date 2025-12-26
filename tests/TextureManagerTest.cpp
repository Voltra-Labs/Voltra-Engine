#include <gtest/gtest.h>
#include "Renderer/TextureManager.hpp"

class TextureManagerTest : public ::testing::Test {
protected:
    void SetUp() override {
        Voltra::TextureManager::Init();
    }

    void TearDown() override {
        Voltra::TextureManager::Clean();
    }
};

TEST_F(TextureManagerTest, LoadNonExistentReturnsDefault) {
    auto tex = Voltra::TextureManager::GetTexture("ruta/falsa/no_existe.png");
    
    ASSERT_NE(tex, nullptr);
    EXPECT_EQ(tex->GetWidth(), 1);
    EXPECT_EQ(tex->GetHeight(), 1);
}

TEST_F(TextureManagerTest, CachingWorks) {
    std::string path = "assets/missing_texture.png";
    
    auto tex1 = Voltra::TextureManager::GetTexture(path);
    auto tex2 = Voltra::TextureManager::GetTexture(path);
    
    EXPECT_EQ(tex1, tex2);
}