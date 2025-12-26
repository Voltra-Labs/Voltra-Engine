#include "Scene.hpp"
#include "Entity.hpp"
#include "Components.hpp"
#include "Renderer/Renderer.hpp"
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include "Renderer/Renderer2D.hpp"

#include <box2d/box2d.h>

namespace Voltra {

    Scene::Scene() {
    }

    Scene::~Scene() {
    }

    Entity Scene::CreateEntity(const std::string& name) {
        Entity entity = { m_Registry.create(), this };
        entity.AddComponent<TransformComponent>();
        auto& tag = entity.AddComponent<TagComponent>();
        tag.Tag = name.empty() ? "Entity" : name;
        return entity;
    }

    void Scene::DestroyEntity(Entity entity) {
        m_Registry.destroy(entity);
    }

    static b2BodyType VoltraBodyTypeToBox2D(Rigidbody2DComponent::BodyType bodyType) {
        switch (bodyType) {
            case Rigidbody2DComponent::BodyType::Static:    return b2_staticBody;
            case Rigidbody2DComponent::BodyType::Dynamic:   return b2_dynamicBody;
            case Rigidbody2DComponent::BodyType::Kinematic: return b2_kinematicBody;
        }
        return b2_staticBody;
    }

    void Scene::OnRuntimeStart() {
        m_PhysicsWorld = new b2World({ 0.0f, -9.8f });

        auto view = m_Registry.view<Rigidbody2DComponent>();
        for (auto e : view) {
            Entity entity = { e, this };
            auto& transform = entity.GetComponent<TransformComponent>();
            auto& rb = entity.GetComponent<Rigidbody2DComponent>();

            // Extract Transform
            glm::vec3 scale;
            scale.x = glm::length(glm::vec3(transform.Transform[0]));
            scale.y = glm::length(glm::vec3(transform.Transform[1]));

            // Extract Rotation (Z-axis for 2D)
            float angle = glm::atan(transform.Transform[0][1], transform.Transform[0][0]);

            b2BodyDef bodyDef;
            bodyDef.type = VoltraBodyTypeToBox2D(rb.Type);
            bodyDef.position.Set(transform.Transform[3][0], transform.Transform[3][1]);
            bodyDef.angle = angle;

            b2Body* body = m_PhysicsWorld->CreateBody(&bodyDef);
            body->SetFixedRotation(rb.FixedRotation);
            rb.RuntimeBody = body;

            if (entity.HasComponent<BoxCollider2DComponent>()) {
                auto& bc = entity.GetComponent<BoxCollider2DComponent>();

                b2PolygonShape boxShape;
                boxShape.SetAsBox(bc.Size.x * scale.x, bc.Size.y * scale.y, b2Vec2(bc.Offset.x, bc.Offset.y), 0.0f); 
                
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

    void Scene::OnRuntimeStop() {
        delete m_PhysicsWorld;
        m_PhysicsWorld = nullptr;
    }

    void Scene::OnUpdate(Timestep ts) {
        // Update Physics
        const int32_t velocityIterations = 6;
        const int32_t positionIterations = 2;
        if (m_PhysicsWorld) {
            m_PhysicsWorld->Step(ts, velocityIterations, positionIterations);

            // Sync back to Transform
            auto view = m_Registry.view<Rigidbody2DComponent>();
            for (auto e : view) {
                Entity entity = { e, this };
                auto& rb = entity.GetComponent<Rigidbody2DComponent>();
                
                if (rb.RuntimeBody) {
                    b2Body* body = (b2Body*)rb.RuntimeBody;
                    const auto& position = body->GetPosition();
                    auto& transform = entity.GetComponent<TransformComponent>();
                    transform.Transform[3][0] = position.x;
                    transform.Transform[3][1] = position.y;
                    
                    float angle = body->GetAngle();

                    // Retrieve existing scale
                    glm::vec3 scale;
                    scale.x = glm::length(glm::vec3(transform.Transform[0]));
                    scale.y = glm::length(glm::vec3(transform.Transform[1]));
                    scale.z = glm::length(glm::vec3(transform.Transform[2]));

                    transform.Transform = glm::translate(glm::mat4(1.0f), glm::vec3(position.x, position.y, transform.Transform[3][2]))
                        * glm::rotate(glm::mat4(1.0f), angle, glm::vec3(0.0f, 0.0f, 1.0f))
                        * glm::scale(glm::mat4(1.0f), scale);
                }
            }
        }

        // Scripts (Logic)
        m_Registry.view<NativeScriptComponent>().each([=](auto entity, auto& nsc) {
            if (!nsc.Instance) {
                nsc.Instance = nsc.InstantiateScript();
                nsc.Instance->m_Entity = Entity{ entity, this };
                nsc.Instance->OnCreate();
            }
            nsc.Instance->OnUpdate(ts);
        });

        // Rendering
        CameraComponent* mainCamera = nullptr;
        glm::mat4 cameraTransform;

        auto view = m_Registry.view<TransformComponent, CameraComponent>();
        for (auto entity : view) {
            auto [transform, camera] = view.get<TransformComponent, CameraComponent>(entity);
            if (camera.Primary) {
                mainCamera = &camera;
                cameraTransform = transform.Transform;
                break;
            }
        }

        if (mainCamera) {
            // Synchronize camera position
            glm::vec3 pos = glm::vec3(cameraTransform[3]);
            mainCamera->Camera.SetPosition(pos);
            
            Renderer2D::BeginScene(mainCamera->Camera);

            // Draw all entities with SpriteRenderer and Transform
            auto group = m_Registry.group<TransformComponent>(entt::get<SpriteRendererComponent>);
            for (auto entity : group) {
                auto [transform, sprite] = group.get<TransformComponent, SpriteRendererComponent>(entity);
                
                // If it has a texture, we use the texture overload
                if (sprite.Texture) {
                    // TODO: DrawQuad(transform, texture, tint)
                    Renderer2D::DrawQuad(transform.Transform, sprite.Texture);
                } 
                else {
                    Renderer2D::DrawQuad(transform.Transform, sprite.Color);
                }
            }

            Renderer2D::EndScene();
        }
    }

}