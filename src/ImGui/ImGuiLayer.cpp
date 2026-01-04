#include "ImGuiLayer.hpp"

#include <imgui.h>
#include <backends/imgui_impl_glfw.h>
#include <backends/imgui_impl_opengl3.h>

#include "Core/Application.hpp"
#include "Core/Log.hpp"

#include <GLFW/glfw3.h>
#include <glad/glad.h>

namespace Voltra {

    /**
     * @brief Constructs the ImGuiLayer with a debug name.
     */
    ImGuiLayer::ImGuiLayer() 
        : Layer("ImGuiLayer") {}

    /**
     * @brief Destroys the ImGuiLayer.
     */
    ImGuiLayer::~ImGuiLayer() {}

    /**
     * @brief Configures ImGui context, styles, fonts, and backends.
     */
    void ImGuiLayer::OnAttach() {
        IMGUI_CHECKVERSION();
        ImGui::CreateContext();
        ImGuiIO& io = ImGui::GetIO(); (void)io;
        io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
        io.ConfigFlags |= ImGuiConfigFlags_DockingEnable;
        
        io.Fonts->AddFontFromFileTTF("../assets/fonts/Roboto-Regular.ttf", 18.0f);
        io.FontDefault = io.Fonts->Fonts.back();

        ImGui::StyleColorsDark();

        ImGuiStyle& style = ImGui::GetStyle();
        if (io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable) {
            style.WindowRounding = 0.0f;
            style.Colors[ImGuiCol_WindowBg].w = 1.0f;
        }

        Application& app = Application::Get();
        GLFWwindow* window = app.GetWindow().GetNativeWindow();

        ImGui_ImplGlfw_InitForOpenGL(window, true);
        ImGui_ImplOpenGL3_Init("#version 410");
        
        VOLTRA_CORE_INFO("ImGuiLayer attached.");
    }

    /**
     * @brief Cleans up ImGui context and backends.
     */
    void ImGuiLayer::OnDetach() {
        ImGui_ImplOpenGL3_Shutdown();
        ImGui_ImplGlfw_Shutdown();
        ImGui::DestroyContext();
        VOLTRA_CORE_INFO("ImGuiLayer detached.");
    }

    /**
     * @brief Called during the ImGui render phase of the layer stack.
     */
    void ImGuiLayer::OnImGuiRender() {
        static bool show = true;
    }

    /**
     * @brief Starts a new ImGui frame context.
     */
    void ImGuiLayer::Begin() {
        ImGui_ImplOpenGL3_NewFrame();
        ImGui_ImplGlfw_NewFrame();
        ImGui::NewFrame();
    }

    /**
     * @brief Renders the collected ImGui draw data and handles viewports.
     */
    void ImGuiLayer::End() {
        ImGuiIO& io = ImGui::GetIO();
        Application& app = Application::Get();
        io.DisplaySize = ImVec2((float)app.GetWindow().GetWidth(), (float)app.GetWindow().GetHeight());

        ImGui::Render();
        ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

        if (io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable) {
            GLFWwindow* backup_current_context = glfwGetCurrentContext();
            ImGui::UpdatePlatformWindows();
            ImGui::RenderPlatformWindowsDefault();
            glfwMakeContextCurrent(backup_current_context);
        }
    }

    /**
     * @brief Blocks events if ImGui wants to capture inputs.
     * 
     * @param event The event to check and potentially handle.
     */
    void ImGuiLayer::OnEvent(Event& event) {
        ImGuiIO& io = ImGui::GetIO();
        event.Handled |= event.IsInCategory(EventCategoryMouse) & io.WantCaptureMouse;
        event.Handled |= event.IsInCategory(EventCategoryKeyboard) & io.WantCaptureKeyboard;
    }
}
