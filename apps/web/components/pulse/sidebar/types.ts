export type SidebarSectionId =
  | 'extracted'
  | 'starred'
  | 'recents'
  | 'tags'
  | 'templates'
  | 'workspace'

export interface SidebarSection {
  id: SidebarSectionId
  label: string
}

export interface StarredItem {
  url: string
  title: string
  starredAt: number
}

export interface RecentItem {
  url: string
  title: string
  accessedAt: number
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
