#include "Scene.hpp"
#include "Entity.hpp"
#include "Components.hpp"
#include "Renderer/Renderer.hpp"
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include "Renderer/Renderer2D.hpp"

#include <box2d/box2d.h>

#include "Renderer/EditorCamera.hpp"


namespace Voltra {

    /**
     * @brief Scene Constructor.
     */
    Scene::Scene() {
    }

    /**
     * @brief Scene Destructor.
     */
    Scene::~Scene() {
    }

    /**
     * @brief Creates an entity with default Transform and Tag components.
     * 
     * @param name The tag name.
     * @return The created Entity handle.
     */
    Entity Scene::CreateEntity(const std::string& name, UUID uuid) {
        Entity entity = { m_Registry.create(), this };
        entity.AddComponent<IDComponent>(uuid);
        entity.AddComponent<TransformComponent>();
        auto& tag = entity.AddComponent<TagComponent>();
        tag.Tag = name.empty() ? "Entity" : name;
        return entity;
    }

    /**
     * @brief Removes the entity from the registry.
     * 
     * @param entity The entity to destroy.
     */
    void Scene::DestroyEntity(Entity entity) {
        m_Registry.destroy(entity);
    }

    /**
     * @brief Iterates components to find the primary camera.
     * 
     * @return The primary camera entity, or null entity if none found.
     */
    Entity Scene::GetPrimaryCameraEntity() {
        auto view = m_Registry.view<CameraComponent>();
        for (auto entity : view) {
            const auto& camera = view.get<CameraComponent>(entity);
            if (camera.Primary)
                return Entity{ entity, this };
        }
        return {};
    }

    /**
     * @brief Helper to convert Voltra BodyType to Box2D BodyType.
     * 
     * @param bodyType Internal enum.
     * @return Box2D enum.
     */
    static b2BodyType VoltraBodyTypeToBox2D(Rigidbody2DComponent::BodyType bodyType) {
        switch (bodyType) {
            case Rigidbody2DComponent::BodyType::Static:    return b2_staticBody;
            case Rigidbody2DComponent::BodyType::Dynamic:   return b2_dynamicBody;
            case Rigidbody2DComponent::BodyType::Kinematic: return b2_kinematicBody;
        }
        return b2_staticBody;
    }

    /**
     * @brief Instantiates the Physics World and creates bodies for all rigidbodies.
     */
    void Scene::OnRuntimeStart() {
        m_PhysicsWorld = new b2World({ 0.0f, -9.8f });

        auto view = m_Registry.view<Rigidbody2DComponent>();
        for (auto e : view) {
            Entity entity = { e, this };
            auto& transform = entity.GetComponent<TransformComponent>();
            auto& rb = entity.GetComponent<Rigidbody2DComponent>();

            b2BodyDef bodyDef;
            bodyDef.type = VoltraBodyTypeToBox2D(rb.Type);
            bodyDef.position.Set(transform.Translation.x, transform.Translation.y);
            bodyDef.angle = transform.Rotation.z;

            b2Body* body = m_PhysicsWorld->CreateBody(&bodyDef);
            body->SetFixedRotation(rb.FixedRotation);
            rb.RuntimeBody = body;

            if (entity.HasComponent<BoxCollider2DComponent>()) {
                auto& bc = entity.GetComponent<BoxCollider2DComponent>();

                b2PolygonShape boxShape;
                boxShape.SetAsBox(bc.Size.x * transform.Scale.x, bc.Size.y * transform.Scale.y, b2Vec2(bc.Offset.x, bc.Offset.y), 0.0f); 
                
                b2FixtureDef fixtureDef;
                fixtureDef.shape = &boxShape;
                fixtureDef.density = bc.Density;
                fixtureDef.friction = bc.Friction;
                fixtureDef.restitution = bc.Restitution;
                fixtureDef.restitutionThreshold = bc.RestitutionThreshold;
                
                body->CreateFixture(&fixtureDef);
            }
        }
    }

    /**
     * @brief Destroys the Physics World.
     */
    void Scene::OnRuntimeStop() {
        delete m_PhysicsWorld;
        m_PhysicsWorld = nullptr;
    }

    /**
     * @brief Steps physics world, updates scripts, and renders the scene.
     * 
     * @param ts Time step.
     */
    void Scene::OnUpdate(Timestep ts) {
        const int32_t velocityIterations = 6;
        const int32_t positionIterations = 2;
        if (m_PhysicsWorld) {
            m_PhysicsWorld->Step(ts, velocityIterations, positionIterations);

            auto view = m_Registry.view<Rigidbody2DComponent>();
            for (auto e : view) {
                Entity entity = { e, this };
                auto& rb = entity.GetComponent<Rigidbody2DComponent>();
                
                if (rb.RuntimeBody) {
                    b2Body* body = (b2Body*)rb.RuntimeBody;
                    const auto& position = body->GetPosition();
                    auto& transform = entity.GetComponent<TransformComponent>();
                    transform.Translation.x = position.x;
                    transform.Translation.y = position.y;
                    transform.Rotation.z = body->GetAngle();
                }
            }
        }

        m_Registry.view<NativeScriptComponent>().each([=](auto entity, auto& nsc) {
            if (!nsc.Instance) {
                nsc.Instance = nsc.InstantiateScript();
                nsc.Instance->m_Entity = Entity{ entity, this };
                nsc.Instance->OnCreate();
            }
            nsc.Instance->OnUpdate(ts);
        });

        CameraComponent* mainCamera = nullptr;
        glm::mat4 cameraTransform;

        auto view = m_Registry.view<TransformComponent, CameraComponent>();
        for (auto entity : view) {
            auto [transform, camera] = view.get<TransformComponent, CameraComponent>(entity);
            if (camera.Primary) {
                mainCamera = &camera;
                cameraTransform = transform.GetTransform();
                break;
            }
        }

        if (mainCamera) {
            glm::vec3 pos = glm::vec3(cameraTransform[3]);
            mainCamera->Camera.SetPosition(pos);
            
            Renderer2D::BeginScene(mainCamera->Camera);

            auto group = m_Registry.group<TransformComponent>(entt::get<SpriteRendererComponent>);
            for (auto entity : group) {
                auto [transform, sprite] = group.get<TransformComponent, SpriteRendererComponent>(entity);
                
                if (sprite.Texture) {
                    // TODO: DrawQuad(transform, texture, tint)
                    Renderer2D::DrawQuad(transform.GetTransform(), sprite.Texture);
                } 
                else {
                    Renderer2D::DrawQuad(transform.GetTransform(), sprite.Color);
                }
            }

            Renderer2D::EndScene();
        }
    }

    /**
     * @brief Renders the scene from the perspective of an editor camera.
     * 
     * @param ts Time step.
     * @param camera The editor camera.
     */
    void Scene::OnUpdateEditor(Timestep ts, EditorCamera& camera) {
        Renderer2D::BeginScene(camera);

        auto group = m_Registry.group<TransformComponent>(entt::get<SpriteRendererComponent>);
        for (auto entity : group) {
            auto [transform, sprite] = group.get<TransformComponent, SpriteRendererComponent>(entity);
            
            if (sprite.Texture) {
                Renderer2D::DrawQuad(transform.GetTransform(), sprite.Texture);
            } 
            else {
                Renderer2D::DrawQuad(transform.GetTransform(), sprite.Color);
            }
        }

        Renderer2D::EndScene();
    }

    /**
     * @brief Renders the scene from the perspective of the primary runtime camera (without logic updates).
     * 
     * @param ts Time step.
     */
    void Scene::OnRenderRuntime(Timestep ts) {
        CameraComponent* mainCamera = nullptr;
        glm::mat4 cameraTransform;

        auto view = m_Registry.view<TransformComponent, CameraComponent>();
        for (auto entity : view) {
            auto [transform, camera] = view.get<TransformComponent, CameraComponent>(entity);
            if (camera.Primary) {
                mainCamera = &camera;
                cameraTransform = transform.GetTransform();
                break;
            }
        }

        if (mainCamera) {
            glm::vec3 pos = glm::vec3(cameraTransform[3]);
            mainCamera->Camera.SetPosition(pos);
            
            Renderer2D::BeginScene(mainCamera->Camera);

            auto group = m_Registry.group<TransformComponent>(entt::get<SpriteRendererComponent>);
            for (auto entity : group) {
                auto [transform, sprite] = group.get<TransformComponent, SpriteRendererComponent>(entity);
                
                if (sprite.Texture) {
                    Renderer2D::DrawQuad(transform.GetTransform(), sprite.Texture);
                } 
                else {
                    Renderer2D::DrawQuad(transform.GetTransform(), sprite.Color);
                }
            }

            Renderer2D::EndScene();
        }
    }

    /**
     * @brief Recalculates camera projection matrices when the viewport resizes.
     * 
     * @param width New width.
     * @param height New height.
     */
    void Scene::OnViewportResize(uint32_t width, uint32_t height)
    {
        m_ViewportWidth = width;
        m_ViewportHeight = height;

        auto view = m_Registry.view<CameraComponent>();
        for (auto entity : view)
        {
            auto& cameraComponent = view.get<CameraComponent>(entity);
            if (!cameraComponent.FixedAspectRatio)
            {
                float aspectRatio = (float)width / (float)height;
                float orthoSize = cameraComponent.OrthographicSize;
                cameraComponent.Camera.SetProjection(-orthoSize * aspectRatio * 0.5f, orthoSize * aspectRatio * 0.5f, -orthoSize * 0.5f, orthoSize * 0.5f);
            }
        }
    }

}
