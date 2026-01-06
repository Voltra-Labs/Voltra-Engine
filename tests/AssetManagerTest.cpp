#include <gtest/gtest.h>
#include "Renderer/AssetManager.hpp"
#include "Renderer/Texture.hpp"

namespace Voltra {

    TEST(AssetManagerTest, TextureCaching) {
        
        std::string path = "assets/textures/test_texture.png";

        std::shared_ptr<Texture2D> tex1 = AssetManager::LoadTexture(path);

        std::shared_ptr<Texture2D> tex2 = AssetManager::LoadTexture(path);

        EXPECT_EQ(tex1, tex2);
        std::string path2 = "assets/textures/other.png";
        std::shared_ptr<Texture2D> tex3 = AssetManager::LoadTexture(path2);
        
    }

}
