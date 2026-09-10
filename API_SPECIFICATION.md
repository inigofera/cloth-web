# Cloth CLI — Initial API Specification v0.1.0

> A comprehensive API design for a CLI tool that manages clothing items and outfits.  
> Derived from the `cloth-web` repository functionality.

---

## Table of Contents

1. [Overview](#overview)
2. [Current cloth-web Functionality](#current-cloth-web-functionality)  
3. [Proposed CLI Data Models](#proposed-cli-data-models)  
4. [Initial Public API (v0.1.0)](#initial-public-api-v010)
5. [API Operations Summary](#api-operations-summary)  
6. [Future Considerations](#future-considerations)

---

## Overview

This document distills the functionality of the `cloth-web` project into a structured initial public API for a **CLI tool** that manages:

- **Clothing Items** — Individual garments with metadata (name, type, color, images, etc.)
- **Outfits** — Collections of clothing items that form a complete look
- **Images/Files** — Photo storage for clothing items

The goal is to define a stable **v0.1.0 API surface** for versioning, ensuring backward compatibility as the project evolves.

---

## Current cloth-web Functionality

### Backend (Rust/Axum)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/users` | GET | List all users from `auth.users` table |
| `/tables` | GET | List all public tables with column names and data types |
| `/db/:table` | GET | Fetch rows from a specified table (max 100 rows) |
| `/db/:table` | POST | Insert a new row into a table |
| `/db/:table/:id` | PATCH | Update an existing row by ID |
| `/db/:table/:id` | DELETE | Delete a row by ID |
| `/upload` | POST | Upload a file to Supabase Storage and return public URL |

### Key Architectural Elements

1. **Generic CRUD** — Dynamic table operations using `json_populate_record` for type coercion
2. **Schema Discovery** — Introspection via `information_schema` to list tables/columns
3. **File Storage** — Supabase Storage integration with public URL generation
4. **PostgreSQL Backend** — Using sqlx with Postgres
5. **UUID-based IDs** — All entities use UUID primary keys

### Frontend (Vue 3 + TypeScript)

- Displays all public database tables with their schemas
- Shows first 5 rows per table as a preview
- Real-time column type information display

---

## Proposed CLI Data Models

### ClothingItem

```typescript
interface ClothingItem {
  id: UUID;                      // Unique identifier
  name: string;                  // Display name (e.g., "Blue Denim Jacket")
  description?: string;          // Optional detailed description
  type: ClothingType;            // Category (see enum below)
  color?: string;                // Primary color
  brand?: string;                // Brand name
  size?: string;                 // Size (S, M, L, XL, or specific measurements)
  material?: string;             // Fabric/material composition
  image_url?: string;            // Public URL to item image
  tags?: string[];               // Custom tags for filtering
  created_at: DateTime;          // Creation timestamp
  updated_at: DateTime;          // Last modification timestamp
}
```

### ClothingType (Enum)

```typescript
enum ClothingType {
  TOP = "top",                   // Shirts, blouses, sweaters
  BOTTOM = "bottom",             // Pants, shorts, skirts
  OUTERWEAR = "outerwear",       // Jackets, coats, vests
  FOOTWEAR = "footwear",         // Shoes, boots, sandals
  ACCESSORY = "accessory",       // Hats, belts, jewelry, bags
  UNDERWEAR = "underwear",       // Undergarments, socks
  ACTIVEWEAR = "activewear",     // Sportswear, gym clothes
  FORMAL = "formal",             // Suits, dresses, ties
  OTHER = "other"                // Miscellaneous
}
```

### Outfit

```typescript
interface Outfit {
  id: UUID;                      // Unique identifier
  name: string;                  // Display name (e.g., "Casual Friday Look")
  description?: string;          // Optional description
  occasion?: string;             // When to wear (casual, work, party, etc.)
  season?: Season;               // Seasonal appropriateness
  image_url?: string;            // Cover image or styled photo
  items: UUID[];                 // Array of ClothingItem IDs in this outfit
  tags?: string[];               // Custom tags for filtering
  created_at: DateTime;          // Creation timestamp
  updated_at: DateTime;          // Last modification timestamp
}
```

### Season (Enum)

```typescript
enum Season {
  SPRING = "spring",
  SUMMER = "summer",
  FALL = "fall",
  WINTER = "winter",
  ALL_SEASON = "all_season"
}
```

### User (Optional for multi-user support)

```typescript
interface User {
  id: UUID;
  email?: string;
  created_at: DateTime;
}
```

---

## Initial Public API (v0.1.0)

All endpoints return JSON. Errors follow a consistent format:

```typescript
interface ApiError {
  error: string;      // Error code
  message: string;    // Human-readable message
  details?: unknown;  // Optional additional context
}
```

### Base URL Structure

```
/api/v1/...
```

---

### 1. Clothing Items API

#### List All Clothing Items

```http
GET /api/v1/items
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `type` | string | Filter by clothing type |
| `color` | string | Filter by color |
| `tags` | string | Comma-separated tags to filter |
| `limit` | integer | Max results (default: 50, max: 100) |
| `offset` | integer | Pagination offset |
| `sort` | string | Sort field (name, created_at, updated_at) |
| `order` | string | Sort order (asc, desc) |

**Response:** `200 OK`
```json
{
  "items": [ClothingItem],
  "total": 42,
  "limit": 50,
  "offset": 0
}
```

#### Get Single Clothing Item

```http
GET /api/v1/items/:id
```

**Response:** `200 OK`
```json
{
  "item": ClothingItem
}
```

**Error:** `404 Not Found` if item doesn't exist

#### Create Clothing Item

```http
POST /api/v1/items
```

**Request Body:**
```json
{
  "name": "Blue Denim Jacket",
  "type": "outerwear",
  "color": "blue",
  "brand": "Levi's",
  "size": "M",
  "material": "100% cotton denim",
  "tags": ["casual", "denim", "spring"]
}
```

**Response:** `201 Created`
```json
{
  "item": ClothingItem
}
```

#### Update Clothing Item

```http
PATCH /api/v1/items/:id
```

**Request Body:** (partial update, only include fields to change)
```json
{
  "color": "light blue",
  "tags": ["casual", "denim", "spring", "favorite"]
}
```

**Response:** `200 OK`
```json
{
  "item": ClothingItem
}
```

#### Delete Clothing Item

```http
DELETE /api/v1/items/:id
```

**Response:** `200 OK`
```json
{
  "deleted": ClothingItem
}
```

**Note:** Returns the deleted item for confirmation. Will fail if item is part of any outfit.

#### Bulk Operations

```http
POST /api/v1/items/bulk
```

**Request Body:**
```json
{
  "operation": "delete" | "update" | "tag",
  "ids": [UUID],
  "data": {
    // For update/tag: fields to apply
  }
}
```

---

### 2. Outfits API

#### List All Outfits

```http
GET /api/v1/outfits
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `occasion` | string | Filter by occasion |
| `season` | string | Filter by season |
| `tags` | string | Comma-separated tags to filter |
| `contains_item` | UUID | Filter outfits containing a specific item |
| `limit` | integer | Max results (default: 50) |
| `offset` | integer | Pagination offset |

**Response:** `200 OK`
```json
{
  "outfits": [Outfit],
  "total": 15,
  "limit": 50,
  "offset": 0
}
```

#### Get Single Outfit

```http
GET /api/v1/outfits/:id
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `expand_items` | boolean | Include full ClothingItem objects (default: false) |

**Response:** `200 OK`
```json
{
  "outfit": Outfit,
  "expanded_items": [ClothingItem]  // Only if expand_items=true
}
```

#### Create Outfit

```http
POST /api/v1/outfits
```

**Request Body:**
```json
{
  "name": "Business Casual Friday",
  "description": "Professional but relaxed office look",
  "occasion": "work",
  "season": "all_season",
  "items": ["uuid-1", "uuid-2", "uuid-3"],
  "tags": ["office", "professional"]
}
```

**Response:** `201 Created`
```json
{
  "outfit": Outfit
}
```

#### Update Outfit

```http
PATCH /api/v1/outfits/:id
```

**Request Body:** (partial update)
```json
{
  "name": "Smart Casual Friday",
  "tags": ["office", "casual", "relaxed"]
}
```

**Response:** `200 OK`
```json
{
  "outfit": Outfit
}
```

#### Delete Outfit

```http
DELETE /api/v1/outfits/:id
```

**Response:** `200 OK`
```json
{
  "deleted": Outfit
}
```

#### Add Item to Outfit

```http
POST /api/v1/outfits/:id/items
```

**Request Body:**
```json
{
  "item_id": "uuid-of-clothing-item"
}
```

**Response:** `200 OK`
```json
{
  "outfit": Outfit
}
```

#### Remove Item from Outfit

```http
DELETE /api/v1/outfits/:id/items/:item_id
```

**Response:** `200 OK`
```json
{
  "outfit": Outfit
}
```

---

### 3. Files/Images API

#### Upload Image

```http
POST /api/v1/images/upload
Content-Type: multipart/form-data
```

**Form Fields:**
| Field | Type | Description |
|-------|------|-------------|
| `file` | File | Image file (jpg, png, webp) |
| `entity_type` | string | "item" or "outfit" |
| `entity_id` | UUID | ID of the item/outfit to associate |

**Response:** `201 Created`
```json
{
  "url": "https://storage.example.com/images/abc123.jpg",
  "filename": "abc123.jpg",
  "size": 245678,
  "mime_type": "image/jpeg"
}
```

#### Get Image URL

```http
GET /api/v1/images/:filename
```

**Response:** `302 Redirect` to the public URL

---

### 4. Schema/Metadata API

#### Get API Info

```http
GET /api/v1/info
```

**Response:** `200 OK`
```json
{
  "version": "0.1.0",
  "entities": ["items", "outfits", "images"],
  "clothing_types": ["top", "bottom", "outerwear", ...],
  "seasons": ["spring", "summer", "fall", "winter", "all_season"]
}
```

#### Get Statistics

```http
GET /api/v1/stats
```

**Response:** `200 OK`
```json
{
  "total_items": 127,
  "total_outfits": 23,
  "items_by_type": {
    "top": 45,
    "bottom": 32,
    "footwear": 18,
    ...
  },
  "outfits_by_season": {
    "summer": 8,
    "winter": 6,
    ...
  }
}
```

---

## API Operations Summary

| Category | Operation | Method | Endpoint |
|----------|-----------|--------|----------|
| **Items** | List all | GET | `/api/v1/items` |
| | Get one | GET | `/api/v1/items/:id` |
| | Create | POST | `/api/v1/items` |
| | Update | PATCH | `/api/v1/items/:id` |
| | Delete | DELETE | `/api/v1/items/:id` |
| | Bulk ops | POST | `/api/v1/items/bulk` |
| **Outfits** | List all | GET | `/api/v1/outfits` |
| | Get one | GET | `/api/v1/outfits/:id` |
| | Create | POST | `/api/v1/outfits` |
| | Update | PATCH | `/api/v1/outfits/:id` |
| | Delete | DELETE | `/api/v1/outfits/:id` |
| | Add item | POST | `/api/v1/outfits/:id/items` |
| | Remove item | DELETE | `/api/v1/outfits/:id/items/:item_id` |
| **Images** | Upload | POST | `/api/v1/images/upload` |
| | Get URL | GET | `/api/v1/images/:filename` |
| **Meta** | API info | GET | `/api/v1/info` |
| | Statistics | GET | `/api/v1/stats` |

**Total: 16 endpoints** for initial v0.1.0 release

---

## CLI Command Mapping

For the CLI tool, these API operations map to commands:

```bash
# Items
cloth items list [--type=TYPE] [--color=COLOR] [--tags=TAG1,TAG2]
cloth items get <ID>
cloth items add --name="..." --type=TYPE [--color=...] [--brand=...]
cloth items update <ID> [--name=...] [--color=...]
cloth items delete <ID>
cloth items tag <ID> --add=TAG | --remove=TAG

# Outfits
cloth outfits list [--season=SEASON] [--occasion=OCCASION]
cloth outfits get <ID> [--expand]
cloth outfits create --name="..." [--occasion=...] [--season=...]
cloth outfits update <ID> [--name=...] [--season=...]
cloth outfits delete <ID>
cloth outfits add-item <OUTFIT_ID> <ITEM_ID>
cloth outfits remove-item <OUTFIT_ID> <ITEM_ID>

# Images
cloth image upload <FILE> --for=item|outfit --id=<ID>

# General
cloth info
cloth stats
```

---

## Future Considerations

### v0.2.0 Potential Additions

- **Search** — Full-text search across items and outfits
- **Wardrobe organization** — Categories, sections, locations
- **Wear tracking** — Log when items are worn, last worn date
- **Weather integration** — Suggest outfits based on weather
- **Import/Export** — Backup and restore wardrobe data

### v0.3.0 Potential Additions

- **Multi-user support** — Different wardrobes per user
- **Sharing** — Share outfits with others
- **Recommendations** — AI-powered outfit suggestions
- **Laundry tracking** — Clean/dirty status

### Breaking Change Policy

Starting with v0.1.0:
- Minor versions (0.x.0): May add new endpoints, new optional fields
- Breaking changes: New major version only
- Deprecation: Minimum 2 minor versions notice before removal

---

## Appendix: Mapping from cloth-web

| cloth-web Feature | CLI API Equivalent |
|-------------------|-------------------|
| `GET /tables` | `GET /api/v1/info` — expose entity metadata |
| `GET /db/:table` | `GET /api/v1/items`, `GET /api/v1/outfits` |
| `POST /db/:table` | `POST /api/v1/items`, `POST /api/v1/outfits` |
| `PATCH /db/:table/:id` | `PATCH /api/v1/items/:id`, `PATCH /api/v1/outfits/:id` |
| `DELETE /db/:table/:id` | `DELETE /api/v1/items/:id`, `DELETE /api/v1/outfits/:id` |
| `POST /upload` | `POST /api/v1/images/upload` |
| Schema introspection | Predefined schemas in CLI types |
| Generic JSON values | Strongly-typed models |

---

*Generated from cloth-web repository analysis*  
*API Version: 0.1.0*  
*Document Version: 1.0*
