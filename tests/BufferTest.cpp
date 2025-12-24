#include <gtest/gtest.h>
#include "Renderer/Buffer.hpp"

// Helper to access private members if needed, or just test public API
// BufferLayout API is public, so we can test directly.

TEST(BufferLayout, ShaderDataTypeSize) {
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Float), 4);
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Float2), 8);
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Float3), 12);
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Float4), 16);
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Mat3), 36); // 4 * 3 * 3
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Mat4), 64); // 4 * 4 * 4
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Int), 4);
    EXPECT_EQ(Voltra::ShaderDataTypeSize(Voltra::ShaderDataType::Bool), 1);
}

TEST(BufferLayout, ElementComponentCount) {
    Voltra::BufferElement elForFloat3(Voltra::ShaderDataType::Float3, "Position");
    EXPECT_EQ(elForFloat3.GetComponentCount(), 3);

    Voltra::BufferElement elForMat4(Voltra::ShaderDataType::Mat4, "ViewProjection");
    EXPECT_EQ(elForMat4.GetComponentCount(), 16); // 4*4
    
    Voltra::BufferElement elForBool(Voltra::ShaderDataType::Bool, "Visible");
    EXPECT_EQ(elForBool.GetComponentCount(), 1);
}

TEST(BufferLayout, LayoutStrideAndOffsets) {
    // Define a layout: Position (Float3), Color (Float4), TexCoord (Float2)
    Voltra::BufferLayout layout = {
        { Voltra::ShaderDataType::Float3, "Position" },
        { Voltra::ShaderDataType::Float4, "Color" },
        { Voltra::ShaderDataType::Float2, "TexCoord" }
    };

    const auto& elements = layout.GetElements();
    ASSERT_EQ(elements.size(), 3);

    // Check sizes and offsets
    // 1. Position: Size 12, Offset 0
    EXPECT_EQ(elements[0].Size, 12);
    EXPECT_EQ(elements[0].Offset, 0);

    // 2. Color: Size 16, Offset 12
    EXPECT_EQ(elements[1].Size, 16);
    EXPECT_EQ(elements[1].Offset, 12);

    // 3. TexCoord: Size 8, Offset 12 + 16 = 28
    EXPECT_EQ(elements[2].Size, 8);
    EXPECT_EQ(elements[2].Offset, 28);

    // Total Stride should be 28 + 8 = 36
    EXPECT_EQ(layout.GetStride(), 36);
}

TEST(BufferLayout, EmptyLayout) {
    Voltra::BufferLayout layout;
    EXPECT_EQ(layout.GetStride(), 0);
    EXPECT_TRUE(layout.GetElements().empty());
}
