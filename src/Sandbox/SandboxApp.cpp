#include <Core/Application.hpp>
#include <Sandbox/EditorLayer.hpp>
#include <ImGui/ImGuiLayer.hpp>
#include <imgui.h>

namespace Voltra {

    class Sandbox : public Application {
    public:
        Sandbox() {
            // Synchronize ImGui context between Core DLL and Sandbox DLL
            ImGui::SetCurrentContext(Get().GetImGuiLayer()->GetContext());
            
            PushLayer(new EditorLayer());
        }

        ~Sandbox() {
        }
    };

}

extern "C" VOLTRA_CLIENT_API Voltra::Application* CreateApplication() {
    return new Voltra::Sandbox();
}
