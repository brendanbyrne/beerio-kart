export interface SimpleItem {
  id: number
  name: string
  image_path: string
}

export interface TrackItem {
  id: number
  name: string
  cup_id: number
  position: number
  image_path: string
}

export interface CupWithTracks {
  id: number
  name: string
  image_path: string
  tracks: TrackItem[]
}

export interface DrinkType {
  id: string
  name: string
  alcoholic: boolean
  created_by: string | null
  created_at: string
}

export interface DrinkTypeInfo {
  id: string
  name: string
  alcoholic: boolean
}

export interface UserPublicProfile {
  id: string
  username: string
  preferred_character_id: number | null
  preferred_body_id: number | null
  preferred_wheel_id: number | null
  preferred_glider_id: number | null
  preferred_drink_type_id: string | null
  created_at: string
}

export interface UserDetailProfile {
  id: string
  username: string
  preferred_character_id: number | null
  preferred_body_id: number | null
  preferred_wheel_id: number | null
  preferred_glider_id: number | null
  preferred_drink_type: DrinkTypeInfo | null
  created_at: string
}

export interface RaceSetup {
  preferred_character_id: number
  preferred_body_id: number
  preferred_wheel_id: number
  preferred_glider_id: number
}
