/**
 * 语言模型配置（Tauri 项目版）
 * 图标用 emoji 代替，避免额外图片资源管理
 */

export interface ModelConfig {
  /** 模型标识，传递给 Dify API */
  id: string
  /** 显示名称 */
  name: string
  /** emoji 图标 */
  emoji: string
  /** 模型描述 */
  description: string
}

/**
 * 可用模型列表
 */
export const AVAILABLE_MODELS: ModelConfig[] = [
  {
    id: 'qwen',
    name: '通义千问',
    emoji: '🤖',
    description: '同规模业界SOTA水平'
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    emoji: '🧠',
    description: '混合推理架构模型'
  }
]

/**
 * 默认模型
 */
export const DEFAULT_MODEL = 'qwen'

/**
 * 根据模型 ID 获取模型配置
 */
export function getModelById(modelId: string): ModelConfig | undefined {
  return AVAILABLE_MODELS.find(model => model.id === modelId)
}

/**
 * 默认模型配置（用于回退）
 */
export const DEFAULT_MODEL_CONFIG: ModelConfig = AVAILABLE_MODELS[0]!

/**
 * 获取当前选中的模型
 * 从 localStorage 读取设置
 */
export function getSelectedModel(): ModelConfig {
  try {
    const saved = localStorage.getItem('aidi-settings')
    if (saved) {
      const settings = JSON.parse(saved)
      const modelId = settings.selectedModel || DEFAULT_MODEL
      return getModelById(modelId) ?? DEFAULT_MODEL_CONFIG
    }
  } catch {
    // ignore errors
  }
  return DEFAULT_MODEL_CONFIG
}
