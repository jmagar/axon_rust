export type SidebarSectionId = 'extracted' | 'workspace'

export interface SidebarSection {
  id: SidebarSectionId
  label: string
}
