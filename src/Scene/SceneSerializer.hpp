#pragma once

#include "Scene/Scene.hpp"
#include <memory>
#include <string>

namespace Voltra {

    /**
     * @brief Serializes and deserializes Scenes to/from YAML files.
     */
    class SceneSerializer {
    public:
        /**
         * @brief Constructs a serializer for a specific scene.
         * 
         * @param scene The scene to manage.
         */
        SceneSerializer(const std::shared_ptr<Scene>& scene);

        /**
         * @brief Saves the scene to a file in text format (YAML).
         * 
         * @param filepath Destination path.
         */
        void Serialize(const std::string& filepath);

        /**
         * @brief Saves the scene validation binary data (Not Implemented).
         * 
         * @param filepath Destination path.
         */
        void SerializeRuntime(const std::string& filepath);

        /**
         * @brief Loads a scene from a YAML file.
         * 
         * @param filepath Source path.
         * @return true if successful.
         */
        bool Deserialize(const std::string& filepath);

        /**
         * @brief Loads a runtime binary scene (Not Implemented).
         * 
         * @param filepath Source path.
         * @return true if successful.
         */
        bool DeserializeRuntime(const std::string& filepath);

    private:
        std::shared_ptr<Scene> m_Scene;
    };

}
