export type SidebarSectionId = 'extracted' | 'workspace'

export interface SidebarSection {
  id: SidebarSectionId
  label: string
}

export interface TagDef {
  id: string
  name: string
  color: string
}

export interface TaggedItem {
  url: string
  tagIds: string[]
}
