#include <gtest/gtest.h>
#include "Core/LayerStack.hpp"
#include "Core/Layer.hpp"

class TestLayer : public Voltra::Layer {
public:
    TestLayer(const std::string& name) : Voltra::Layer(name) {}
};

TEST(LayerStack, PushLayerAndOverlayOrder) {
    Voltra::LayerStack stack;
    TestLayer* layer1 = new TestLayer("Layer1");
    TestLayer* layer2 = new TestLayer("Layer2");
    TestLayer* overlay1 = new TestLayer("Overlay1");

    stack.PushLayer(layer1);
    stack.PushOverlay(overlay1); 
    stack.PushLayer(layer2);     

    auto it = stack.begin();
    EXPECT_EQ(*it, layer1);
    it++;
    EXPECT_EQ(*it, layer2);
    it++;
    EXPECT_EQ(*it, overlay1);

}

TEST(LayerStack, PopLayer) {
    Voltra::LayerStack stack;
    TestLayer* layer1 = new TestLayer("Layer1");
    
    stack.PushLayer(layer1);
    stack.PopLayer(layer1);
    
    EXPECT_EQ(stack.begin(), stack.end());
    delete layer1;
}